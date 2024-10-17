# options_tokenization

This is a smart contract(Solana Program) for minting, transferring, and exercising options on the Solana blockchain, developed using **Anchor**.

## Features:
- **Minting Options**: Users can mint call/put options as SPL tokens, representing rights to buy (call) or sell (put) an underlying asset.
- **Partial Option Exercise**: Users can exercise part of their options, allowing them to access only a portion of the underlying assets.
- **American/European Options**: Supports both American options (can be exercised anytime before expiration) and European options (can only be exercised at expiration).
- **Option Cancellation**: Option creators can cancel their options before expiration and retrieve their locked assets.

## Project Structure:

### 1. **Program (lib.rs)**:
- **Status**: Fully implemented and functional.
- **Description**: This Rust-based Solana program, built using Anchor, handles the minting, transferring, exercising, and cancellation of options. It manages the underlying assets through an escrow system and supports both call and put options with the ability to exercise partially or fully.
  
### 2. **Test Suite (anchor.test.ts)**:
- **Status**: Still in development.
- **Description**: Tests are written to validate the key functionalities of the program, including:
  - Minting options.
  - Transferring options.
  - Exercising options (both partial and full exercises).
  - Canceling options before expiration.
- **TODO**: Expand the tests to cover more edge cases, such as handling expired options and invalid operations.

### 3. **Client File (client.ts)**:
- **Status**: Planned, not yet implemented.
- **Description**: The client file will contain scripts to interact with the deployed smart contract, making it easier for users or front-end applications to interact with the Solana options program.


## Current Status:
- **Core Contract (lib.rs)**: Fully implemented and functional.
- **Test Suite**: Partially functional, with more tests to be developed.
- **Client File**: Will be completed if needed for this project.

  ## Tech Stack:
  Anchor, Solana, Rust and Solana Playground IDE
