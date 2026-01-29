import * as anchor from '@coral-xyz/anchor'
import { Program, BN } from '@coral-xyz/anchor'
import { FundraisingProgram } from '../target/types/fundraising_program'
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js'
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAssociatedTokenAddress,
  getAccount,
} from '@solana/spl-token'
import { assert, expect } from 'chai'
import { createHash } from 'crypto'
// @ts-ignore
import { timelockEncrypt, timelockDecrypt, mainnetClient, HttpChainClient } from 'tlock-js'
// @ts-ignore
import { fetchBeacon } from 'drand-client'

function sha256(data: Buffer): Buffer {
  return createHash('sha256').update(data).digest()
}

function computeMerkleLeaf(userPubkey: PublicKey, score: number): Buffer {
  const scoreBuffer = Buffer.alloc(4)
  scoreBuffer.writeUInt32LE(score, 0)
  const data = Buffer.concat([userPubkey.toBuffer(), scoreBuffer])
  return sha256(data)
}

function hashNodes(left: Buffer, right: Buffer): Buffer {
  const [first, second] = Buffer.compare(left, right) <= 0 ? [left, right] : [right, left]
  return sha256(Buffer.concat([first, second]))
}

function buildMerkleTree(leaves: Buffer[]): {
  root: Buffer
  getProof: (leafIndex: number) => Buffer[]
} {
  if (leaves.length === 0) {
    throw new Error('Cannot build tree with no leaves')
  }

  const levels: Buffer[][] = [leaves.slice()]

  while (levels[levels.length - 1].length > 1) {
    const currentLevel = levels[levels.length - 1]
    const nextLevel: Buffer[] = []

    for (let i = 0; i < currentLevel.length; i += 2) {
      if (i + 1 < currentLevel.length) {
        nextLevel.push(hashNodes(currentLevel[i], currentLevel[i + 1]))
      } else {
        nextLevel.push(currentLevel[i])
      }
    }

    levels.push(nextLevel)
  }

  const root = levels[levels.length - 1][0]

  function getProof(leafIndex: number): Buffer[] {
    const proof: Buffer[] = []
    let index = leafIndex

    for (let level = 0; level < levels.length - 1; level++) {
      const currentLevel = levels[level]
      const siblingIndex = index % 2 === 0 ? index + 1 : index - 1

      if (siblingIndex < currentLevel.length) {
        proof.push(currentLevel[siblingIndex])
      }

      index = Math.floor(index / 2)
    }

    return proof
  }

  return { root, getProof }
}

function proofToAnchorFormat(proof: Buffer[]): number[][] {
  return proof.map((node) => Array.from(node))
}

function getSealedBidPda(programId: PublicKey, salePda: PublicKey, userPubkey: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([Buffer.from('sealed_bid'), salePda.toBuffer(), userPubkey.toBuffer()], programId)
}

const QUICKNET_CHAIN_HASH = '52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971'

let drandClient: HttpChainClient

async function initDrandClient() {
  console.log('  Initializing drand client...')
  drandClient = mainnetClient()
  console.log('  ✓ Drand client initialized (mainnet/quicknet)')
}

async function getCurrentDrandRound(): Promise<number> {
  const beacon = await fetchBeacon(drandClient)
  return beacon.round
}

async function encryptBid(maxFdv: number, round: number): Promise<Buffer> {
  console.log(`  Encrypting max_fdv=$${(maxFdv / 1e6).toFixed(0)} for round ${round}...`)

  // Encode the max_fdv as a buffer
  const message = Buffer.alloc(8)
  message.writeBigUInt64LE(BigInt(maxFdv))

  const ciphertext = await timelockEncrypt(round, message, drandClient)

  console.log(`  ✓ Encrypted bid (${ciphertext.length} bytes)`)
  return Buffer.from(ciphertext)
}

async function fetchDrandBeacon(round: number) {
  console.log(`  Fetching drand beacon for round ${round}...`)
  const beacon = await fetchBeacon(drandClient, round)
  console.log(`  ✓ Beacon fetched`)
  return beacon
}

async function fetchDrandSignature(round: number): Promise<Buffer> {
  const beacon = await fetchDrandBeacon(round)
  const signature = Buffer.from(beacon.signature, 'hex')
  console.log(`  ✓ Fetched drand signature (${signature.length} bytes)`)
  return signature
}

async function decryptBid(ciphertext: Buffer, round: number): Promise<number> {
  console.log(`  Decrypting bid for round ${round}...`)
  const ciphertextStr = ciphertext.toString()
  const decrypted = await timelockDecrypt(ciphertextStr, drandClient)
  const maxFdv = Number(Buffer.from(decrypted).readBigUInt64LE(0))
  console.log(`  ✓ Decrypted max_fdv=$${(maxFdv / 1e6).toFixed(0)}`)
  return maxFdv
}

async function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function getBlockTime(connection: anchor.web3.Connection): Promise<number> {
  const slot = await connection.getSlot()
  const blockTime = await connection.getBlockTime(slot)
  return blockTime || Math.floor(Date.now() / 1000)
}

async function advanceTime(connection: anchor.web3.Connection, wallet: Keypair, numBlocks: number = 10): Promise<void> {
  for (let i = 0; i < numBlocks; i++) {
    const dummyTx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: wallet.publicKey,
        lamports: 1,
      }),
    )
    await anchor.web3.sendAndConfirmTransaction(connection, dummyTx, [wallet])
  }
}

describe('V0.2-Drand Sealed-Bid Auction', () => {
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider)

  const program = anchor.workspace.FundraisingProgram as Program<FundraisingProgram>
  const connection = provider.connection
  const authority = (provider.wallet as anchor.Wallet).payer

  let tokenMint: PublicKey
  let usdcMint: PublicKey
  let salePda: PublicKey
  let usdcVault: PublicKey
  let tokenVault: PublicKey
  let authorityTokenAccount: PublicKey

  // Test users
  const NUM_USERS = 3
  const users: Keypair[] = []
  const userUsdcAccounts: PublicKey[] = []
  const userSealedBidPdas: PublicKey[] = []
  let merkleTree: ReturnType<typeof buildMerkleTree>
  const userScores = new Map<string, { score: number; index: number }>()

  let commitmentStart: number
  let commitmentEnd: number
  let drandRevealRound: number

  // Sale parameters
  const testBids = [
    { userIndex: 0, amount: 100_000, maxFdv: 2_000_000, score: 100 },
    { userIndex: 1, amount: 150_000, maxFdv: 1_500_000, score: 100 },
    { userIndex: 2, amount: 80_000, maxFdv: 1_000_000, score: 100 },
  ]

  const CLEARING_FDV = new BN(1_000_000 * 1e6)

  before(async () => {
    console.log('\n=== V0.2-Drand Setup ===\n')

    // Initialize drand client
    await initDrandClient()

    // Generate test users
    console.log('Generating test users...')
    for (let i = 0; i < NUM_USERS; i++) {
      users.push(Keypair.generate())
    }
    console.log(`✓ Generated ${NUM_USERS} users`)

    // Build Merkle tree
    console.log('Building Merkle tree...')
    const leaves: Buffer[] = []
    testBids.forEach((bid, index) => {
      const user = users[bid.userIndex]
      const score = bid.score
      userScores.set(user.publicKey.toBase58(), { score, index })
      const leaf = computeMerkleLeaf(user.publicKey, score)
      leaves.push(leaf)
    })
    merkleTree = buildMerkleTree(leaves)

    console.log(`✓ Merkle root: ${merkleTree.root.toString('hex')}`)

    // Create mints
    tokenMint = await createMint(connection, authority, authority.publicKey, null, 6)
    console.log(`Token mint: ${tokenMint.toBase58()}`)

    usdcMint = await createMint(connection, authority, authority.publicKey, null, 6)
    console.log(`USDC mint: ${usdcMint.toBase58()}`)

    // Create authority token account and mint tokens
    authorityTokenAccount = await createAssociatedTokenAccount(connection, authority, tokenMint, authority.publicKey)

    await mintTo(connection, authority, tokenMint, authorityTokenAccount, authority.publicKey, 1_000_000_000 * 1e6)

    // Fund users with SOL and USDC
    for (let i = 0; i < NUM_USERS; i++) {
      await connection.requestAirdrop(users[i].publicKey, 2 * LAMPORTS_PER_SOL)
      await sleep(500)

      const userUsdcAccount = await createAssociatedTokenAccount(connection, authority, usdcMint, users[i].publicKey)
      userUsdcAccounts.push(userUsdcAccount)

      await mintTo(connection, authority, usdcMint, userUsdcAccount, authority.publicKey, 500_000 * 1e6)

      console.log(`User ${i} funded with SOL and USDC`)
    }

    console.log('\n✓ Setup complete\n')
  })

  it('Should initialize sale with drand configuration', async () => {
    console.log('\n=== Test 1: Initialize Sale ===')

    const COMMITMENT_WINDOW_SECONDS = 900 // 15 minutes
    const realTimeNow = Math.floor(Date.now() / 1000)
    commitmentStart = realTimeNow + 10 // Start 10 seconds from now
    commitmentEnd = commitmentStart + COMMITMENT_WINDOW_SECONDS

    // Calculate drand reveal round from commitment_end
    // Quicknet: genesis = 1692803367, period = 3 seconds
    const QUICKNET_GENESIS = 1692803367
    const QUICKNET_PERIOD = 3
    drandRevealRound = Math.ceil((commitmentEnd - QUICKNET_GENESIS) / QUICKNET_PERIOD)

    // Verify the round is in the future
    const currentDrandRound = await getCurrentDrandRound()
    const roundsUntilReveal = drandRevealRound - currentDrandRound
    const secondsUntilReveal = roundsUntilReveal * QUICKNET_PERIOD

    console.log(`Real-world time now: ${realTimeNow}`)
    console.log(`Commitment window: ${commitmentStart} → ${commitmentEnd} (${COMMITMENT_WINDOW_SECONDS}s)`)
    console.log(`Current drand round: ${currentDrandRound}`)
    console.log(`Drand reveal round: ${drandRevealRound} (${roundsUntilReveal} rounds / ~${secondsUntilReveal}s in future)`)
    console.log(`\nPRODUCTION-LIKE TEST: Will wait ~${Math.ceil(secondsUntilReveal / 60)} minutes for drand beacon`)
    ;[salePda] = PublicKey.findProgramAddressSync(
      [Buffer.from('sale'), authority.publicKey.toBuffer(), tokenMint.toBuffer()],
      program.programId,
    )

    usdcVault = await getAssociatedTokenAddress(usdcMint, salePda, true)
    tokenVault = await getAssociatedTokenAddress(tokenMint, salePda, true)

    console.log('Initializing sale...')
    const tx = await program.methods
      .initializeSale(
        'Drand Sealed-Bid Sale',
        new BN(1_000_000_000 * 1e6),
        4000,
        new BN(100_000 * 1e6),
        new BN(500_000 * 1e6),
        new BN(400_000 * 1e6),
        new BN(2_000_000 * 1e6),
        new BN(commitmentStart),
        new BN(commitmentEnd),
        Array.from(merkleTree.root),
      )
      .accounts({
        sale: salePda,
        authority: authority.publicKey,
        tokenMint: tokenMint,
        usdcMint: usdcMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc()

    console.log(`✓ Initialize Sale TX: ${tx.substring(0, 16)}...`)

    const sale = await program.account.sale.fetch(salePda)
    // Fetch the drand reveal round calculated by the program
    drandRevealRound = sale.drandRevealRound.toNumber()
    assert.equal(sale.allBidsRevealed, false)

    console.log(`✓ Sale initialized with drand reveal round: ${drandRevealRound}`)
    console.log(`  Sale PDA: ${salePda.toBase58()}`)
  })

  it('Should fund the sale', async () => {
    console.log('\n=== Test 1.5: Fund Sale ===')
    console.log('Funding sale with 400M tokens...')

    const tx = await program.methods
      .fundSale(new BN(400_000_000 * 1e6))
      .accounts({
        sale: salePda,
        authority: authority.publicKey,
        tokenMint: tokenMint,
        tokenVault: tokenVault,
        authorityTokenAccount: authorityTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc()

    const sale = await program.account.sale.fetch(salePda)
    expect(sale.status).to.have.property('active')

    console.log(`✓ Sale funded and activated`)
    console.log(`  TX: ${tx.substring(0, 16)}...`)
  })

  it('Should allow users to commit with encrypted bids', async () => {
    console.log('\n=== Test 2: User Commitments with Encrypted Bids ===')
    console.log('Waiting for commitment window to open...')
    let currentTime = await getBlockTime(connection)

    if (currentTime < commitmentStart) {
      console.log(`  Current time: ${currentTime}, window opens: ${commitmentStart}`)
      while (currentTime < commitmentStart) {
        await advanceTime(connection, authority, 5)
        await sleep(500)
        currentTime = await getBlockTime(connection)
        console.log(`  Current time: ${currentTime}, window opens: ${commitmentStart}`)
      }
    }
    console.log(`✓ Commitment window is now open (current time: ${currentTime})`)

    console.log(`\nProcessing ${testBids.length} encrypted commitments...`)
    for (let i = 0; i < testBids.length; i++) {
      console.log(`\n[User ${i}] Starting commitment process...`)
      const bid = testBids[i]
      const user = users[bid.userIndex]
      const userUsdcAccount = userUsdcAccounts[bid.userIndex]
      const [sealedBidPda] = getSealedBidPda(program.programId, salePda, user.publicKey)
      userSealedBidPdas.push(sealedBidPda)

      const userInfo = userScores.get(user.publicKey.toBase58())!
      const proof = merkleTree.getProof(userInfo.index)
      const proofArrays = proofToAnchorFormat(proof)

      // Real timelock encryption
      console.log(`[User ${i}] Encrypting bid with drand...`)
      const encryptedBid = await encryptBid(bid.maxFdv * 1e6, drandRevealRound)
      console.log(`[User ${i}] ✓ Bid encrypted successfully`)

      console.log(`[User ${i}] Submitting commit transaction...`)
      const tx = await program.methods
        .commit(new BN(bid.amount * 1e6), encryptedBid, new BN(drandRevealRound), bid.score, proofArrays as any)
        .accounts({
          sale: salePda,
          sealedBid: sealedBidPda,
          user: user.publicKey,
          userUsdcAccount: userUsdcAccount,
          usdcVault: usdcVault,
          usdcMint: usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc()

      console.log(`[User ${i}] ✓ Committed $${bid.amount.toLocaleString()} with encrypted bid`)
      console.log(`[User ${i}] TX: ${tx.substring(0, 16)}...`)
    }

    const sale = await program.account.sale.fetch(salePda)
    assert.equal(sale.totalUsers, testBids.length)

    console.log(`\n✓ All ${testBids.length} users committed with encrypted bids`)
  })

  it('Should close commitment window', async () => {
    console.log('\n=== Test 3: Close Commitment Window ===')

    let realTimeNow = Math.floor(Date.now() / 1000)

    if (realTimeNow < commitmentEnd) {
      const waitSeconds = commitmentEnd - realTimeNow + 5
      console.log(`Waiting ${Math.ceil(waitSeconds / 60)} minutes for commitment window to close (real time)...`)
      console.log(`  Real time now: ${realTimeNow}, window ends: ${commitmentEnd}`)

      while (realTimeNow < commitmentEnd) {
        await sleep(30000) // 30 seconds
        realTimeNow = Math.floor(Date.now() / 1000)
        const remaining = commitmentEnd - realTimeNow
        if (remaining > 0) {
          console.log(`  ⏳ ${Math.ceil(remaining / 60)} minutes remaining...`)
        }
      }
    }
    console.log(`✓ Commitment window closed (real time: ${realTimeNow})`)

    console.log('Closing commitment window...')
    const tx = await program.methods
      .closeCommitmentWindow()
      .accounts({
        sale: salePda,
        closer: authority.publicKey,
      })
      .rpc()

    const sale = await program.account.sale.fetch(salePda)
    expect(sale.status).to.have.property('revealing')

    console.log(`✓ Commitment window closed → Revealing status`)
    console.log(`  TX: ${tx.substring(0, 16)}...`)
  })

  it('Should batch reveal encrypted bids', async () => {
    console.log('\n=== Test 4: Batch Reveal Bids ===')

    const currentRound = await getCurrentDrandRound()
    if (currentRound < drandRevealRound) {
      const roundsToWait = drandRevealRound - currentRound
      const secondsToWait = roundsToWait * 3 + 5 // 3s per round
      console.log(`⏳ Waiting ${secondsToWait}s for drand round ${drandRevealRound} (current: ${currentRound})...`)
      await sleep(secondsToWait * 1000)
    }

    console.log('Fetching real drand signature...')
    const drandSig = await fetchDrandSignature(drandRevealRound)
    console.log(`✓ Drand signature fetched for round ${drandRevealRound}`)

    // Fetch all encrypted bids and decrypt them
    console.log('\nDecrypting bids off-chain...')
    const revealedBids = []
    for (let i = 0; i < testBids.length; i++) {
      const bid = await program.account.sealedBid.fetch(userSealedBidPdas[i])
      console.log(`[User ${i}] Decrypting bid...`)

      const encryptedData = Buffer.from(bid.maxFdvEncrypted)
      const decryptedMaxFdv = await decryptBid(encryptedData, drandRevealRound)

      revealedBids.push({
        user: users[testBids[i].userIndex].publicKey,
        maxFdv: new BN(decryptedMaxFdv),
      })
      console.log(`[User ${i}] ✓ Decrypted: $${(decryptedMaxFdv / 1e6).toFixed(0)}`)
    }

    console.log('\nSubmitting batch reveal transaction...')
    const remainingAccounts = userSealedBidPdas.map((pda) => ({
      pubkey: pda,
      isSigner: false,
      isWritable: true,
    }))

    const tx = await program.methods
      .batchRevealBids(new BN(drandRevealRound), drandSig, revealedBids)
      .accounts({
        sale: salePda,
        cranker: authority.publicKey,
      })
      .remainingAccounts(remainingAccounts)
      .rpc()

    console.log(`✓ Batch reveal TX: ${tx.substring(0, 16)}...`)

    const sale = await program.account.sale.fetch(salePda)
    assert.equal(sale.revealCount, testBids.length)
    assert.equal(sale.allBidsRevealed, true)
    expect(sale.status).to.have.property('commitmentEnded')

    // Verify bids are revealed on-chain
    console.log('\nVerifying revealed bids on-chain:')
    for (let i = 0; i < testBids.length; i++) {
      const bid = await program.account.sealedBid.fetch(userSealedBidPdas[i])
      assert.equal(bid.bidRevealed, true)
      assert.ok(bid.maxFdvPlaintext !== null)
      console.log(`  User ${i}: $${(bid.maxFdvPlaintext.toNumber() / 1e6).toFixed(0)} FDV`)
    }

    console.log('\n✓ All bids revealed successfully!')
  })

  it('Should propose settlement', async () => {
    console.log('\n=== Test 5: Propose Settlement ===')

    const INITIAL_PROPOSAL_DELAY = 30
    const proposalAllowedTime = commitmentEnd + INITIAL_PROPOSAL_DELAY
    let realTimeNow = Math.floor(Date.now() / 1000)

    if (realTimeNow < proposalAllowedTime) {
      const waitSeconds = proposalAllowedTime - realTimeNow + 2 // +2s buffer
      console.log(`⏳ Waiting ${waitSeconds}s for initial proposal delay...`)
      await sleep(waitSeconds * 1000)
      realTimeNow = Math.floor(Date.now() / 1000)
    }
    console.log(`Initial proposal delay passed (real time: ${realTimeNow})`)

    console.log(`Proposing settlement at clearing FDV: $${(CLEARING_FDV.toNumber() / 1e6).toFixed(0)}`)

    const tx = await program.methods
      .proposeSettlement(CLEARING_FDV, new BN(10000), null, null)
      .accounts({
        sale: salePda,
        authority: authority.publicKey,
      })
      .rpc()

    const sale = await program.account.sale.fetch(salePda)
    expect(sale.status).to.have.property('proposed')

    console.log(`✓ Settlement proposed`)
    console.log(`  TX: ${tx.substring(0, 16)}...`)
  })

  it('Should batch verify and allocate', async () => {
    console.log('\n=== Test 6: Batch Verify and Allocate ===')
    console.log(`Verifying ${testBids.length} users...`)

    const remainingAccounts = userSealedBidPdas.map((pda) => ({
      pubkey: pda,
      isSigner: false,
      isWritable: true,
    }))

    const tx = await program.methods
      .batchVerifyAndAllocate()
      .accounts({
        sale: salePda,
        cranker: authority.publicKey,
      })
      .remainingAccounts(remainingAccounts)
      .rpc()

    const sale = await program.account.sale.fetch(salePda)
    assert.equal(sale.verificationCount, testBids.length)

    console.log(`✓ All ${testBids.length} users verified and allocated`)
    console.log(`  TX: ${tx.substring(0, 16)}...`)
  })

  it('Should finalize settlement', async () => {
    console.log('\n=== Test 7: Finalize Settlement ===')
    console.log('Finalizing settlement...')

    const tx = await program.methods
      .finalizeSettlement()
      .accounts({
        sale: salePda,
        tokenVault: tokenVault,
        finalizer: authority.publicKey,
      })
      .rpc()

    const sale = await program.account.sale.fetch(salePda)
    expect(sale.status).to.have.property('settled')

    console.log(`✓ Settlement finalized`)
    console.log(`  TX: ${tx.substring(0, 16)}...`)
  })

  it('Should enable claims and users can claim', async () => {
    console.log('\n=== Test 8: Enable Claims & Claim Allocation ===')
    console.log('Enabling claims...')

    const enableTx = await program.methods
      .enableClaims()
      .accounts({
        sale: salePda,
        authority: authority.publicKey,
      })
      .rpc()

    console.log(`✓ Claims enabled`)
    console.log(`  TX: ${enableTx.substring(0, 16)}...`)

    // Claim for first user
    console.log('\nUser 0 claiming allocation...')
    const userTokenAccount = await createAssociatedTokenAccount(connection, authority, tokenMint, users[0].publicKey)

    const claimTx = await program.methods
      .claimAllocation()
      .accounts({
        sale: salePda,
        sealedBid: userSealedBidPdas[0],
        user: users[0].publicKey,
        tokenMint: tokenMint,
        userTokenAccount: userTokenAccount,
        tokenVault: tokenVault,
        usdcMint: usdcMint,
        userUsdcAccount: userUsdcAccounts[0],
        usdcVault: usdcVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([users[0]])
      .rpc()

    const tokenBalance = await getAccount(connection, userTokenAccount)
    console.log(`✓ User 0 claimed ${tokenBalance.amount} tokens`)
    console.log(`  TX: ${claimTx.substring(0, 16)}...`)

    console.log('\n=== All V0.2-Drand Tests Complete! ===')
  })
})
