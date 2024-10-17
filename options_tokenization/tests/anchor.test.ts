// No imports needed: web3, anchor, pg and more are globally available
//WILL fix anchor.test.ts and use if needed
const { BN } = require("bn.js");
const assert = require("assert");

describe("Options Tokenization Tests", () => {
  // Keypair for user accounts
  const user = web3.Keypair.generate();
  const feeReceiver = web3.Keypair.generate();
  let mintAccount = null;
  let optionMintAccount = null;
  let userUnderlyingAccount = null;
  let userOptionAccount = null;
  let escrowAccount = null;
  const program = pg.program;

  before(async () => {
    // Airdrop SOL to the user's account for testing
    await pg.connection.requestAirdrop(user.publicKey, 2 * web3.LAMPORTS_PER_SOL);
    await pg.connection.confirmTransaction(await pg.connection.getLatestBlockhash());

    // Create token mint for the underlying asset (e.g., USDC)
    mintAccount = await pg.createMint(user.publicKey);

    // Create the user's token account for the underlying asset
    userUnderlyingAccount = await mintAccount.createAccount(user.publicKey);

    // Mint some tokens (e.g., 100 USDC) to the user's underlying asset account
    await mintAccount.mintTo(userUnderlyingAccount, user.publicKey, [], 100 * web3.LAMPORTS_PER_SOL);
  });

  it("Mint a new option", async () => {
    const strikePrice = new BN(50 * web3.LAMPORTS_PER_SOL);
    const expiration = new BN(Math.floor(Date.now() / 1000) + 60); // Expires in 1 minute
    const amountUnderlying = new BN(100); // 100 units of underlying asset
    const fee = new BN(1 * web3.LAMPORTS_PER_SOL);
    const isAmerican = true;

    // Create new accounts for option mint and escrow
    optionMintAccount = web3.Keypair.generate();
    escrowAccount = web3.Keypair.generate();

    // Create token account for the user to receive the option token
    userOptionAccount = await mintAccount.createAccount(user.publicKey);

    // Mint the option
    const txHash = await program.methods
      .mintOption(strikePrice, expiration, OPTION_TYPE_CALL, amountUnderlying, fee, isAmerican)
      .accounts({
        option: optionMintAccount.publicKey,
        mint: mintAccount.publicKey,
        tokenAccount: userOptionAccount,
        user: user.publicKey,
        underlyingMint: mintAccount.publicKey,
        underlyingAssetAccount: userUnderlyingAccount,
        escrow: escrowAccount.publicKey,
        feeReceiver: feeReceiver.publicKey,
        tokenProgram: web3.TOKEN_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .signers([user, optionMintAccount, escrowAccount])
      .rpc();

    console.log(`Mint option transaction: ${txHash}`);

    // Fetch the minted option account
    const optionData = await program.account.option.fetch(optionMintAccount.publicKey);
    assert(optionData.strikePrice.eq(strikePrice));
    assert(optionData.expiration.eq(expiration));
    assert(optionData.amountUnderlying.eq(amountUnderlying));
    assert(optionData.mintAuthority.equals(user.publicKey));

    console.log("Option successfully minted:", optionData);
  });

  it("Transfer option token", async () => {
    const recipient = web3.Keypair.generate();
    const recipientOptionAccount = await mintAccount.createAccount(recipient.publicKey);

    // Transfer the option token from user to recipient
    const transferTxHash = await program.methods
      .transferOption(new BN(1)) // Transfer 1 option token
      .accounts({
        from: userOptionAccount,
        to: recipientOptionAccount,
        authority: user.publicKey,
        tokenProgram: web3.TOKEN_PROGRAM_ID,
        option: optionMintAccount.publicKey,
      })
      .signers([user])
      .rpc();

    console.log(`Transfer option transaction: ${transferTxHash}`);

    // Fetch and check recipient's token account balance
    const recipientOptionBalance = await mintAccount.getAccountInfo(recipientOptionAccount);
    assert(recipientOptionBalance.amount.eq(new BN(1)));

    console.log("Option successfully transferred to recipient");
  });

  it("Exercise option partially", async () => {
    const amountToExercise = new BN(50); // Partially exercise 50 units

    const exerciseTxHash = await program.methods
      .exerciseOption(amountToExercise)
      .accounts({
        option: optionMintAccount.publicKey,
        mint: mintAccount.publicKey,
        optionTokenAccount: userOptionAccount,
        optionHolder: userUnderlyingAccount,
        escrow: escrowAccount.publicKey,
        escrowAuthority: user.publicKey,
        user: user.publicKey,
        tokenProgram: web3.TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    console.log(`Exercise option transaction: ${exerciseTxHash}`);

    // Fetch updated option data
    const optionData = await program.account.option.fetch(optionMintAccount.publicKey);
    assert(optionData.amountUnderlying.eq(new BN(50))); // 50 units remaining

    console.log("Option successfully partially exercised:", optionData);
  });

  it("Cancel the option before expiration", async () => {
    const cancelTxHash = await program.methods
      .cancelOption()
      .accounts({
        option: optionMintAccount.publicKey,
        user: user.publicKey,
        escrow: escrowAccount.publicKey,
        escrowAuthority: user.publicKey,
        tokenProgram: web3.TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    console.log(`Cancel option transaction: ${cancelTxHash}`);

    // Verify the option is cancelled and underlying assets are returned
    const optionData = await program.account.option.fetch(optionMintAccount.publicKey);
    assert(optionData.isExercised); // Ensure it's marked as canceled

    console.log("Option successfully canceled:", optionData);
  });

  it("Fail to exercise an expired option", async () => {
    // Wait for the option to expire
    await new Promise((resolve) => setTimeout(resolve, 60000)); // 1 minute

    try {
      await program.methods
        .exerciseOption(new BN(10))
        .accounts({
          option: optionMintAccount.publicKey,
          mint: mintAccount.publicKey,
          optionTokenAccount: userOptionAccount,
          optionHolder: userUnderlyingAccount,
          escrow: escrowAccount.publicKey,
          escrowAuthority: user.publicKey,
          user: user.publicKey,
          tokenProgram: web3.TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
    } catch (err) {
      assert(err.message.includes("OptionExpired"));
      console.log("Failed to exercise expired option as expected.");
    }
  });
});
