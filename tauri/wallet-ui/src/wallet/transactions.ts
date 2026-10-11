import { Buffer } from "buffer";
import { Transaction, VersionedTransaction } from "@solana/web3.js";
import nacl from "tweetnacl";
import bs58 from "bs58";

export function deserializeTransaction(bytes: Uint8Array): Transaction | VersionedTransaction {
  const transaction = VersionedTransaction.deserialize(bytes);
  return transaction.message.version === "legacy" ? Transaction.from(bytes) : transaction;
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
    if (!nacl.sign.detached.verify(message, signed.signatures[i], keys[i].toBytes())) {
      throw new Error("The wallet returned an invalid or incomplete signature");
    }
    const previous = original.signatures[i];
    if (previous.some(byte => byte !== 0) && !Buffer.from(previous).equals(Buffer.from(signed.signatures[i]))) {
      throw new Error("The wallet replaced an existing signature");
    }
  }
}
