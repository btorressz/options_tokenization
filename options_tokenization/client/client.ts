import * as anchor from '@project-serum/anchor';
import { web3, Program, Provider } from '@project-serum/anchor';
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

//TODO: EDIT THIS FILE 

// Define the key constants (OPTION_TYPE_CALL = 0, OPTION_TYPE_PUT = 1)
const OPTION_TYPE_CALL = 0;
const OPTION_TYPE_PUT = 1;

// Get provider (This should already be set up in Solana Playground)
const provider = anchor.AnchorProvider.env();
anchor.setProvider(provider);

const program = anchor.workspace.OptionsTokenization as Program<any>;

(async () => {
  try {
    // Wallet address
    const wallet = provider.wallet.publicKey;
    console.log("Wallet address:", wallet.toString());

    // Fetch balance
    const balance = await provider.connection.getBalance(wallet);
    console.log(`Balance: ${balance / web3.LAMPORTS_PER_SOL} SOL`);

    // Choose a function to call based on your action, e.g., mintOption, transferOption, etc.

    //  Mint an Option
    async function mintOption() {
      const optionMintAccount = web3.Keypair.generate();
      const escrowAccount = web3.Keypair.generate();

      // Create accounts for option token and escrow
      const tokenAccount = await createTokenAccount(wallet);
      const underlyingMint = await createMint(wallet);
      const userUnderlyingAccount = await createTokenAccount(wallet, underlyingMint.publicKey);

      // Set parameters for minting the option
      const strikePrice = new anchor.BN(50 * web3.LAMPORTS_PER_SOL);
      const expiration = new anchor.BN(Math.floor(Date.now() / 1000) + 3600); // Expires in 1 hour
      const amountUnderlying = new anchor.BN(100); // 100 units of the underlying asset
      const fee = new anchor.BN(1 * web3.LAMPORTS_PER_SOL); // Fee for minting
      const isAmerican = true; // American option

      // Call the `mintOption` method from the smart contract
      const tx = await program.methods.mintOption(
        strikePrice, expiration, OPTION_TYPE_CALL, amountUnderlying, fee, isAmerican
      ).accounts({
        option: optionMintAccount.publicKey,
        mint: underlyingMint.publicKey,
        tokenAccount: tokenAccount,
        user: wallet,
        underlyingMint: underlyingMint.publicKey,
        underlyingAssetAccount: userUnderlyingAccount,
        escrow: escrowAccount.publicKey,
        feeReceiver: wallet, // You can change this to a fee-receiving address
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([optionMintAccount, escrowAccount])
      .rpc();

      console.log("Mint Option Transaction Signature:", tx);
    }

    // Transfer Option
    async function transferOption(recipientPublicKey: web3.PublicKey) {
      const recipientTokenAccount = await createTokenAccount(recipientPublicKey);

      // Call the `transferOption` method
      const tx = await program.methods.transferOption(new anchor.BN(1)) // Transfer 1 option token
        .accounts({
          from: await findTokenAccount(wallet),
          to: recipientTokenAccount,
          authority: wallet,
          option: await findOptionMintAccount(wallet), // Assume this function fetches the correct option
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      console.log("Transfer Option Transaction Signature:", tx);
    }

    // Exercise Option (Partial or Full)
    async function exerciseOption(amount: number) {
      const optionAccount = await findOptionMintAccount(wallet);
      const escrowAccount = await findEscrowAccount(wallet);
      const userUnderlyingAccount = await findTokenAccount(wallet);

      const tx = await program.methods.exerciseOption(new anchor.BN(amount)) // Specify amount to exercise
        .accounts({
          option: optionAccount,
          mint: await findMintAccount(wallet),
          optionTokenAccount: await findTokenAccount(wallet),
          optionHolder: userUnderlyingAccount,
          escrow: escrowAccount,
          escrowAuthority: wallet,
          user: wallet,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      console.log("Exercise Option Transaction Signature:", tx);
    }

    //  Cancel Option
    async function cancelOption() {
      const optionAccount = await findOptionMintAccount(wallet);
      const escrowAccount = await findEscrowAccount(wallet);

      // Call the `cancelOption` method
      const tx = await program.methods.cancelOption()
        .accounts({
          option: optionAccount,
          user: wallet,
          escrow: escrowAccount,
          escrowAuthority: wallet,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      console.log("Cancel Option Transaction Signature:", tx);
    }

    await mintOption();  

  } catch (err) {
    console.error("Error:", err);
  }
})();


// Helper functions for mint/token accounts
async function createMint(owner: web3.PublicKey) {
  const mint = web3.Keypair.generate();
  await program.provider.connection.confirmTransaction(
    await program.provider.connection.requestAirdrop(mint.publicKey, web3.LAMPORTS_PER_SOL)
  );
  return mint;
}

async function createTokenAccount(owner: web3.PublicKey, mint?: web3.PublicKey) {
  const tokenAccount = web3.Keypair.generate();
  const tokenAccountInfo = await program.provider.connection.getAccountInfo(tokenAccount.publicKey);
  return tokenAccount;
}

async function findTokenAccount(owner: web3.PublicKey) {
  // Implement logic to find token account by owner
  return web3.Keypair.generate().publicKey;
}

async function findOptionMintAccount(owner: web3.PublicKey) {
  // Implement logic to find option mint account for the owner
  return web3.Keypair.generate().publicKey;
}

async function findEscrowAccount(owner: web3.PublicKey) {
  // Implement logic to find escrow account
  return web3.Keypair.generate().publicKey;
}

async function findMintAccount(owner: web3.PublicKey) {
  // Implement logic to find the correct mint account (could be underlying or option)
  return web3.Keypair.generate().publicKey;
}
