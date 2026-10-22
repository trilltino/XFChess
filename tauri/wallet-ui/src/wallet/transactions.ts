import { Buffer } from "buffer";
import { Transaction, VersionedTransaction } from "@solana/web3.js";
import nacl from "tweetnacl";
import bs58 from "bs58";

export function deserializeTransaction(bytes: Uint8Array): Transaction | VersionedTransaction {
  const transaction = VersionedTransaction.deserialize(bytes);
  return transaction.message.version === "legacy" ? Transaction.from(bytes) : transaction;
}

// A wallet may be the first signer of a bundle that the server co-signs later.
// Validate its approval below before accepting these partially signed bytes.
export function serializeWalletTransaction(transaction: Transaction | VersionedTransaction): Uint8Array {
  return transaction instanceof Transaction
    ? transaction.serialize({ requireAllSignatures: false, verifySignatures: false })
    : transaction.serialize();
}

export function assertWalletAddress(actual: string | undefined, expected: string): void {
  if (actual !== expected) throw new Error("Your wallet account changed. Reconnect from the game before signing.");
}

export function validateMessageSignature(messageBase64: string, signedBase64: string, wallet: string): void {
  const signature = Buffer.from(signedBase64, "base64");
  if (signature.length !== 64 || !nacl.sign.detached.verify(Buffer.from(messageBase64, "base64"), signature, bs58.decode(wallet))) {
    throw new Error("The wallet returned an invalid message signature");
  }
}

export function validateSignedTransaction(originalBase64: string, signedBase64: string, wallet: string): void {
  const original = VersionedTransaction.deserialize(Buffer.from(originalBase64, "base64"));
  const signed = VersionedTransaction.deserialize(Buffer.from(signedBase64, "base64"));
  const message = original.message.serialize();
  if (!Buffer.from(message).equals(Buffer.from(signed.message.serialize()))) {
    throw new Error("The wallet changed the requested transaction");
  }
  const keys = original.message.staticAccountKeys;
  const required = original.message.header.numRequiredSignatures;
  const index = keys.slice(0, required).findIndex(key => key.toBase58() === wallet);
  if (index < 0 || signed.signatures.length !== required) throw new Error("This wallet is not a transaction signer");
  for (let i = 0; i < required; i++) {
    const previous = original.signatures[i];
    const signature = signed.signatures[i];
    // Verify this wallet and every supplied signature. An untouched placeholder
    // for another signer is allowed at this stage; the game client separately
    // restricts deferred signatures to its expected server session key.
    const deferred = i !== index && !previous.some(byte => byte !== 0)
      && !signature.some(byte => byte !== 0);
    if (!deferred && !nacl.sign.detached.verify(message, signature, keys[i].toBytes())) {
      throw new Error("The wallet returned an invalid or incomplete signature");
    }
    if (previous.some(byte => byte !== 0) && !Buffer.from(previous).equals(Buffer.from(signed.signatures[i]))) {
      throw new Error("The wallet replaced an existing signature");
    }
  }
}
