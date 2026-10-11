// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import { Buffer } from "buffer";
import { Keypair, Transaction, TransactionMessage, VersionedTransaction, PublicKey } from "@solana/web3.js";
import { assertWalletAddress, validateSignedTransaction, validateMessageSignature } from "./transactions";
import { refreshBlockhash, withTimeout } from "./bridge";
import nacl from "tweetnacl";
import { connectExtension } from "./extension";

const hash = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
const encode = (tx: Transaction | VersionedTransaction) => Buffer.from(tx instanceof Transaction
  ? tx.serialize({ requireAllSignatures: false, verifySignatures: false }) : tx.serialize()).toString("base64");

function fixture(versioned: boolean) {
  const wallet = Keypair.generate();
  const local = Keypair.generate();
  const instruction = {
    programId: new PublicKey(new Uint8Array(32).fill(8)),
    keys: [{ pubkey: local.publicKey, isSigner: true, isWritable: false }],
    data: Buffer.from([1]),
  };
  const tx = versioned ? new VersionedTransaction(new TransactionMessage({
    payerKey: wallet.publicKey, recentBlockhash: hash, instructions: [instruction],
  }).compileToV0Message()) : new Transaction({ feePayer: wallet.publicKey, recentBlockhash: hash }).add(instruction);
  if (tx instanceof Transaction) tx.partialSign(local);
  else tx.sign([local]);
  const original = encode(tx);
  if (tx instanceof Transaction) tx.partialSign(wallet);
  else tx.sign([wallet]);
  return { tx, original, wallet, local };
}

afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

describe.each([false, true])("transaction signing (versioned=%s)", versioned => {
  it("adapts extension signing without losing transaction type or local signatures", async () => {
    const { original, wallet } = fixture(versioned);
    vi.stubGlobal("window", { phantom: { solana: {
      publicKey: wallet.publicKey,
      connect: async () => ({ publicKey: wallet.publicKey }),
      signTransaction: async (tx: Transaction | VersionedTransaction) => {
        expect(tx instanceof VersionedTransaction).toBe(versioned);
        if (tx instanceof Transaction) tx.partialSign(wallet);
        else tx.sign([wallet]);
        return tx;
      },
    } } });
    const source = await connectExtension("phantom");
    const signed = await source.signTransaction(Buffer.from(original, "base64"));
    expect(() => validateSignedTransaction(original, Buffer.from(signed).toString("base64"), source.pubkey)).not.toThrow();
  });
  it("validates wallet approval while preserving the local signature", () => {
    const { tx, original, wallet } = fixture(versioned);
    expect(() => validateSignedTransaction(original, encode(tx), wallet.publicKey.toBase58())).not.toThrow();
  });

  it("rejects a wallet that changes the blockhash", () => {
    const { tx, original, wallet } = fixture(versioned);
    if (tx instanceof Transaction) tx.recentBlockhash = wallet.publicKey.toBase58();
    else tx.message.recentBlockhash = wallet.publicKey.toBase58();
    expect(() => validateSignedTransaction(original, encode(tx), wallet.publicKey.toBase58())).toThrow(/changed/);
  });

  it("rejects missing or invalid signatures", () => {
    const { original, wallet } = fixture(versioned);
    expect(() => validateSignedTransaction(original, original, wallet.publicKey.toBase58())).toThrow(/invalid/);
  });

  it("never refreshes a partially signed transaction", async () => {
    const { original } = fixture(versioned);
    const tx = VersionedTransaction.deserialize(Buffer.from(original, "base64"));
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    expect(await refreshBlockhash(tx)).toBe(false);
    expect(tx.message.recentBlockhash).toBe(hash);
    expect(fetch).not.toHaveBeenCalled();
  });
});

it("refuses signing after the extension switches accounts", () => {
  expect(() => assertWalletAddress("another", "expected")).toThrow(/account changed/);
});

it("validates message signatures against the connected wallet and exact message", () => {
  const wallet = Keypair.generate();
  const message = Buffer.from("xfchess:consent:test");
  const signature = Buffer.from(nacl.sign.detached(message, wallet.secretKey)).toString("base64");
  expect(() => validateMessageSignature(message.toString("base64"), signature, wallet.publicKey.toBase58())).not.toThrow();
  expect(() => validateMessageSignature(Buffer.from("changed").toString("base64"), signature, wallet.publicKey.toBase58())).toThrow(/invalid/);
});

it("clears the timeout after a successful provider call", async () => {
  vi.useFakeTimers();
  await withTimeout(Promise.resolve("signed"), 60000, "Signature");
  expect(vi.getTimerCount()).toBe(0);
});
