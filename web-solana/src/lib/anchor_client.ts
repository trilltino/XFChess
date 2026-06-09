import { AnchorProvider, Program, type Idl, web3 } from '@coral-xyz/anchor';
import { Connection, PublicKey } from '@solana/web3.js';
import idl from './xfchess_game.json';

export const PROGRAM_ID = new PublicKey('8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU');

export function getAnchorProgram(connection: Connection, wallet: any) {
  const provider = new AnchorProvider(connection, wallet as any, {
    preflightCommitment: 'processed',
  });
  return new Program(idl as Idl, provider);
}

export async function fetchPlayerProfile(program: Program, walletPubkey: PublicKey) {
  const [profilePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("profile"), walletPubkey.toBuffer()],
    program.programId
  );
  
  try {
    const profile = await (program.account as any).playerProfile.fetch(profilePda);
    return { pubkey: profilePda, data: profile };
  } catch (err: any) {
    if (err.message && err.message.includes("beyond buffer length")) {
       console.warn("Outdated profile structure detected - re-initialization might be needed.");
    } else {
       console.error("Profile not found:", err);
    }
    return null;
  }
}

export async function createPlayerProfile(
  program: Program,
  walletPubkey: PublicKey,
  username: string,
  country: string,
  dateOfBirth: number, // Unix timestamp in seconds — must be ≥ 18 years before now
) {
  const [profilePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("profile"), walletPubkey.toBuffer()],
    program.programId
  );

  const [usernameRecord] = PublicKey.findProgramAddressSync(
    [Buffer.from("username"), Buffer.from(username)],
    program.programId
  );

  return await (program.methods as any)
    .initProfile(username, country, dateOfBirth)
    .accounts({
      playerProfile: profilePda,
      usernameRecord: usernameRecord,
      player: walletPubkey,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();
}

export async function fetchProfileByUsername(program: Program, username: string) {
  try {
    const [usernameRecord] = PublicKey.findProgramAddressSync(
      [Buffer.from("username"), Buffer.from(username)],
      program.programId
    );
    
    // Fetch the username record to get the wallet pubkey
    const record = await (program.account as any).usernameRecord.fetch(usernameRecord);
    const walletPubkey = new PublicKey(record.owner);
    
    // Now fetch the profile using the wallet pubkey
    return await fetchPlayerProfile(program, walletPubkey);
  } catch (err: any) {
    console.error("Profile not found by username:", err);
    return null;
  }
}
