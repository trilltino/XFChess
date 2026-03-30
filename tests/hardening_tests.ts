import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { XfchessGame } from "../target/types/xfchess_game";
import { expect } from "chai";
import * as crypto from "crypto";

describe("xfchess-game-hardening", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.XfchessGame as Program<XfchessGame>;
  const gameId = new anchor.BN(Math.floor(Math.random() * 1000000));
  
  let gamePda: anchor.web3.PublicKey;
  let moveLogPda: anchor.web3.PublicKey;
  let escrowPda: anchor.web3.PublicKey;

  before(async () => {
    [gamePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("game"), gameId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    [moveLogPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("move_log"), gameId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    [escrowPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("wager_escrow"), gameId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  });

  it("Creates a game and initializes enhanced MoveLog", async () => {
    await program.methods
      .createGame(gameId, new anchor.BN(0), { pvp: {} })
      .accounts({
        game: gamePda,
        moveLog: moveLogPda,
        escrowPda: escrowPda,
        player: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const moveLogAccount = await program.account.moveLog.fetch(moveLogPda);
    expect(moveLogAccount.gameId.toNumber()).to.equal(gameId.toNumber());
    expect(moveLogAccount.nonce.toNumber()).to.equal(0);
    expect(moveLogAccount.moves).to.have.lengthOf(0);
    expect(moveLogAccount.timestamps).to.have.lengthOf(0);
  });

  it("Records a move with nonce validation", async () => {
    const moveStr = "e2e4";
    const nextFen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const nonce = new anchor.BN(1);

    await program.methods
      .recordMove(gameId, moveStr, nextFen, nonce, null)
      .accounts({
        game: gamePda,
        moveLog: moveLogPda,
        player: provider.wallet.publicKey,
      })
      .rpc();

    const moveLogAccount = await program.account.moveLog.fetch(moveLogPda);
    expect(moveLogAccount.nonce.toNumber()).to.equal(1);
    expect(moveLogAccount.moves[0]).to.equal(moveStr);
    expect(moveLogAccount.timestamps).to.have.lengthOf(1);
  });

  it("Fails on invalid nonce (replay protection)", async () => {
    const moveStr = "e7e5";
    const nextFen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";
    const invalidNonce = new anchor.BN(1); // Already used

    try {
      await program.methods
        .recordMove(gameId, moveStr, nextFen, invalidNonce, null)
        .accounts({
          game: gamePda,
          moveLog: moveLogPda,
          player: provider.wallet.publicKey,
        })
        .rpc();
      expect.fail("Should have failed with invalid nonce");
    } catch (e: any) {
        // Anchor error code check
        expect(e.message).to.contain("InvalidNonce");
    }
  });

  it("Can dispute a game", async () => {
    const [disputePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("dispute"), gameId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    const reason = "Suspicious engine use detected";
    const evidenceHash = crypto.createHash('sha256').update("evidence").digest();

    await program.methods
      .disputeGame(gameId, reason, Array.from(evidenceHash))
      .accounts({
        game: gamePda,
        disputeRecord: disputePda,
        player: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const gameAccount = await program.account.game.fetch(gamePda);
    expect(gameAccount.status).to.have.property("disputed");

    const disputeAccount = await program.account.disputeRecord.fetch(disputePda);
    expect(disputeAccount.reason).to.equal(reason);
    expect(disputeAccount.status).to.have.property("pending");
  });

  it("Can cancel a game", async () => {
      // For this test, we might need a new game that is waiting for opponent or inactive
      const newGameId = new anchor.BN(Math.floor(Math.random() * 1000000));
      const [newGamePda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("game"), newGameId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      const [newMoveLogPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("move_log"), newGameId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      const [newEscrowPda] = anchor.web3.PublicKey.findProgramAddressSync(
          [Buffer.from("wager_escrow"), newGameId.toArrayLike(Buffer, "le", 8)],
          program.programId
      );

      await program.methods
        .createGame(newGameId, new anchor.BN(0), { pvp: {} })
        .accounts({
          game: newGamePda,
          moveLog: newMoveLogPda,
          escrowPda: newEscrowPda,
          player: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      await program.methods
        .cancelGame(newGameId)
        .accounts({
          game: newGamePda,
          escrowPda: newEscrowPda,
          player: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      const gameAccount = await program.account.game.fetch(newGamePda);
      expect(gameAccount.status).to.have.property("cancelled");
  });
});
