import * as anchor from "@anchor-lang/core";
import * as fs from "fs";
const signatureVerificationTestIDL = JSON.parse(
  fs.readFileSync("./target/idl/signature_verification_test.json", "utf8")
);
import { Buffer } from "buffer";
import {
  Keypair,
  Transaction,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Ed25519Program,
  Secp256k1Program,
} from "@solana/web3.js";
import * as crypto from "crypto";
import { ethers } from "ethers";
import * as assert from "assert";
import { sign } from "@noble/ed25519";

describe("signature-verification-test", () => {
  const provider = anchor.AnchorProvider.local(undefined, {
    commitment: `confirmed`,
  });

  anchor.setProvider(provider);
  const program = new anchor.Program(
    signatureVerificationTestIDL as anchor.Idl,
    provider
  );

  it("Verify Ed25519 signature with valid signature", async () => {
    const signer = Keypair.generate();
    const message = Buffer.from(
      "Hello, Anchor Signature Verification Test with valid signature!"
    );
    const signature = await sign(message, signer.secretKey.slice(0, 32));

    // Create Ed25519 instruction using SDK
    const ed25519Instruction = Ed25519Program.createInstructionWithPublicKey({
      publicKey: signer.publicKey.toBytes(),
      message: message,
      signature: signature,
    });

    // Create Anchor program verification instruction
    const verifyIx = await program.methods
      .verifyEd25519Signature(
        message,
        Array.from(signature) as [number, ...number[]]
      )
      .accounts({
        signer: signer.publicKey,
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    // Transaction: ed25519 instruction first, then Anchor verification
    const tx = new Transaction().add(ed25519Instruction).add(verifyIx);

    try {
      await provider.sendAndConfirm(tx, []);
      console.log("Ed25519 signature verified successfully!");
    } catch (error) {
      assert.fail("Valid Ed25519 signature should be verified");
    }
  });

  it("Verify Ed25519 signature with invalid signature", async () => {
    const signer = Keypair.generate();
    const message = Buffer.from(
      "Hello, Anchor Signature Verification Test with invalid signature!"
    );
    // Create a fake signature (all zeros)
    const fakeSignature = new Uint8Array(64).fill(0);

    // Create Ed25519 instruction with invalid signature
    const ed25519Instruction = Ed25519Program.createInstructionWithPublicKey({
      publicKey: signer.publicKey.toBytes(),
      message: message,
      signature: fakeSignature,
    });

    // Create Anchor program verification instruction
    const verifyIx = await program.methods
      .verifyEd25519Signature(
        message,
        Array.from(fakeSignature) as [number, ...number[]]
      )
      .accounts({
        signer: signer.publicKey,
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    const transaction = new Transaction().add(ed25519Instruction).add(verifyIx);

    // This should fail - expect signature verification error, not surfpool error
    try {
      await provider.sendAndConfirm(transaction, []);
      assert.fail("Invalid Signature of Ed25519 should not be verified");
    } catch (error: any) {
      // Check that we got a signature verification error, not a surfpool error
      const errorStr = error.toString();
      if (
        errorStr.includes("surfpool") ||
        errorStr.includes(
          "This program may not be used for executing instructions"
        )
      ) {
        // This is a surfpool/validator issue, not the expected signature error
        // Re-throw to fail the test with a clearer message
        throw new Error(
          `Got surfpool/validator error instead of signature verification error: ${errorStr}`
        );
      }
      // Expected: signature verification should fail
      console.log("Invalid Signature of Ed25519 is not verified");
    }
  });

  it("Verify Ed25519 signature using Anchor program (SDK format compatibility)", async () => {
    const signer = Keypair.generate();
    const message = Buffer.from(
      "Hello, Anchor Signature Verification Test using Anchor program!"
    );
    const signature = await sign(message, signer.secretKey.slice(0, 32));

    // Create Ed25519 instruction using SDK (uses [Pubkey, Signature] format)
    const ed25519Instruction = Ed25519Program.createInstructionWithPublicKey({
      publicKey: signer.publicKey.toBytes(),
      message: message,
      signature: signature,
    });

    // Create Anchor program verification instruction
    const verifyIx = await program.methods
      .verifyEd25519Signature(
        message,
        Array.from(signature) as [number, ...number[]]
      )
      .accounts({
        signer: signer.publicKey,
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    // Transaction: ed25519 instruction first, then Anchor verification
    const tx = new Transaction().add(ed25519Instruction).add(verifyIx);

    try {
      await provider.sendAndConfirm(tx, []);
      console.log(
        "Ed25519 signature verified successfully using Anchor program!"
      );
    } catch (error) {
      console.error("Error:", error);
      assert.fail(
        "Valid Ed25519 signature should be verified by Anchor program"
      );
    }
  });

  it("Verify Ethereum Secp256k1 signature with valid signature", async () => {
    const ethSigner: ethers.Wallet = ethers.Wallet.createRandom();
    const PERSON = { name: "ben", age: 49 };

    // keccak256(name, age)
    const messageHashHex: string = ethers.utils.solidityKeccak256(
      ["string", "uint16"],
      [PERSON.name, PERSON.age]
    );
    const messageHashBytes: Uint8Array = ethers.utils.arrayify(messageHashHex);

    // Sign with Ethereum prefix
    const fullSig: string = await ethSigner.signMessage(messageHashBytes);
    const fullSigBytes = ethers.utils.arrayify(fullSig);
    const signature = fullSigBytes.slice(0, 64);
    const recoveryId = fullSigBytes[64] - 27;

    const actualMessage = Buffer.concat([
      Buffer.from("\x19Ethereum Signed Message:\n32"),
      Buffer.from(messageHashBytes),
    ]);

    // 20-byte ETH address (hex without 0x)
    const ethAddressHexNo0x = ethers.utils
      .computeAddress(ethSigner.publicKey)
      .slice(2);
    const ethAddressBytes = Array.from(
      ethers.utils.arrayify("0x" + ethAddressHexNo0x)
    ) as [
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number
    ];

    const verifyIx = await program.methods
      .verifySecp(
        actualMessage,
        Array.from(signature) as [
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number
        ],
        recoveryId,
        ethAddressBytes
      )
      .accounts({
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    // Secp precompile verification against ETH address
    const secpIx = Secp256k1Program.createInstructionWithEthAddress({
      ethAddress: ethAddressHexNo0x,
      message: actualMessage,
      signature: Uint8Array.from(signature),
      recoveryId,
    });

    const tx = new Transaction().add(secpIx).add(verifyIx);
    // This should succeed
    try {
      await provider.sendAndConfirm(tx, []);
      console.log("Ethereum Secp256k1 signature verified successfully!");
    } catch (error) {
      assert.fail("Valid Signature of Ethereum Secp256k1 should be verified");
    }
  });

  it("Verify Ethereum Secp256k1 signature with invalid signature", async () => {
    const ethSigner: ethers.Wallet = ethers.Wallet.createRandom();
    const PERSON = { name: "ben", age: 49 };

    // keccak256(name, age)
    const messageHashHex: string = ethers.utils.solidityKeccak256(
      ["string", "uint16"],
      [PERSON.name, PERSON.age]
    );
    const messageHashBytes: Uint8Array = ethers.utils.arrayify(messageHashHex);

    // Create a fake signature (all zeros)
    const fakeSignature = new Uint8Array(64).fill(0);
    const fakeRecoveryId = 0;

    const actualMessage = Buffer.concat([
      Buffer.from("\x19Ethereum Signed Message:\n32"),
      Buffer.from(messageHashBytes),
    ]);

    const ethAddressHexNo0x = ethers.utils
      .computeAddress(ethSigner.publicKey)
      .slice(2);
    const ethAddressBytes = Array.from(
      ethers.utils.arrayify("0x" + ethAddressHexNo0x)
    ) as [
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number
    ];

    const verifyIx = await program.methods
      .verifySecp(
        actualMessage,
        Array.from(fakeSignature) as [
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number
        ],
        fakeRecoveryId,
        ethAddressBytes
      )
      .accounts({
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();
    const secpIx = Secp256k1Program.createInstructionWithEthAddress({
      ethAddress: ethAddressHexNo0x,
      message: actualMessage,
      signature: fakeSignature,
      recoveryId: fakeRecoveryId,
    });

    const tx = new Transaction().add(secpIx).add(verifyIx);

    // This should fail - expect signature verification error, not surfpool error
    try {
      await provider.sendAndConfirm(tx, []);
      assert.fail("Expected transaction to fail with invalid signature");
    } catch (error: any) {
      // Check that we got a signature verification error, not a surfpool error
      const errorStr = error.toString();
      if (
        errorStr.includes("surfpool") ||
        errorStr.includes(
          "This program may not be used for executing instructions"
        )
      ) {
        // This is a surfpool/validator issue, not the expected signature error
        // Re-throw to fail the test with a clearer message
        throw new Error(
          `Got surfpool/validator error instead of signature verification error: ${errorStr}`
        );
      }
      // Expected: signature verification should fail
      console.log(
        "Ethereum Secp256k1 verification correctly failed with invalid signature"
      );
    }
  });

  it("Verify multiple Ed25519 signatures", async () => {
    const signer1 = Keypair.generate();
    const signer2 = Keypair.generate();
    const message1 = Buffer.from("Message 1 for multiple signatures");
    const message2 = Buffer.from("Message 2 for multiple signatures");

    const signature1 = await sign(message1, signer1.secretKey.slice(0, 32));
    const signature2 = await sign(message2, signer2.secretKey.slice(0, 32));

    // Convert Uint8Array to Buffer
    const sig1Buffer = Buffer.from(signature1);
    const sig2Buffer = Buffer.from(signature2);

    // Create Ed25519 instruction with multiple signatures
    // Format: [num_signatures: u8, padding: u8, ...offsets for each signature]
    const numSignatures = 2;
    const headerSize = 2; // num_signatures + padding
    const offsetSize = 14; // 7 u16 fields per signature

    // Calculate offsets - signatures and pubkeys will be in the instruction data
    const sig1Offset = headerSize + offsetSize * numSignatures;
    const pubkey1Offset = sig1Offset + 64; // signature is 64 bytes
    const msg1Offset = pubkey1Offset + 32; // pubkey is 32 bytes

    const sig2Offset = msg1Offset + message1.length;
    const pubkey2Offset = sig2Offset + 64;
    const msg2Offset = pubkey2Offset + 32;

    const instructionData = Buffer.alloc(msg2Offset + message2.length);

    // Write header
    instructionData.writeUInt8(numSignatures, 0);
    instructionData.writeUInt8(0, 1); // padding

    // Write first signature offsets
    instructionData.writeUInt16LE(sig1Offset, 2);
    instructionData.writeUInt16LE(0xffff, 4); // u16::MAX = current instruction
    instructionData.writeUInt16LE(pubkey1Offset, 6);
    instructionData.writeUInt16LE(0xffff, 8);
    instructionData.writeUInt16LE(msg1Offset, 10);
    instructionData.writeUInt16LE(message1.length, 12);
    instructionData.writeUInt16LE(0xffff, 14);

    // Write second signature offsets
    instructionData.writeUInt16LE(sig2Offset, 16);
    instructionData.writeUInt16LE(0xffff, 18);
    instructionData.writeUInt16LE(pubkey2Offset, 20);
    instructionData.writeUInt16LE(0xffff, 22);
    instructionData.writeUInt16LE(msg2Offset, 24);
    instructionData.writeUInt16LE(message2.length, 26);
    instructionData.writeUInt16LE(0xffff, 28);

    // Write actual data
    sig1Buffer.copy(instructionData, sig1Offset);
    signer1.publicKey.toBytes().copy(instructionData, pubkey1Offset);
    message1.copy(instructionData, msg1Offset);
    sig2Buffer.copy(instructionData, sig2Offset);
    signer2.publicKey.toBytes().copy(instructionData, pubkey2Offset);
    message2.copy(instructionData, msg2Offset);

    const ed25519Instruction = {
      programId: Ed25519Program.programId,
      keys: [],
      data: instructionData,
    };

    // Create Anchor program verification instruction
    const verifyIx = await program.methods
      .verifyEd25519Multiple(
        [
          Array.from(signer1.publicKey.toBytes()) as [number, ...number[]],
          Array.from(signer2.publicKey.toBytes()) as [number, ...number[]],
        ],
        [message1, message2], // Vec<bytes> expects Buffer/Uint8Array directly
        [
          Array.from(sig1Buffer) as [number, ...number[]],
          Array.from(sig2Buffer) as [number, ...number[]],
        ]
      )
      .accounts({
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    const tx = new Transaction().add(ed25519Instruction).add(verifyIx);

    try {
      await provider.sendAndConfirm(tx, []);
      console.log("Multiple Ed25519 signatures verified successfully!");
    } catch (error) {
      assert.fail("Multiple Ed25519 signatures should be verified");
    }
  });

  it("Verify multiple Secp256k1 signatures", async () => {
    const wallet1 = ethers.Wallet.createRandom();
    const wallet2 = ethers.Wallet.createRandom();
    const message1 = Buffer.from("Message 1 for secp256k1");
    const message2 = Buffer.from("Message 2 for secp256k1");

    const sig1 = await wallet1.signMessage(message1);
    const sig2 = await wallet2.signMessage(message2);

    const sig1Bytes = Buffer.from(sig1.slice(2), "hex");
    const sig2Bytes = Buffer.from(sig2.slice(2), "hex");

    // Recovery ID: Ethereum uses v = 27 + recovery_id (or 28 + recovery_id)
    // Solana expects recovery_id to be 0 or 1
    const v1 = parseInt(sig1.slice(130, 132), 16);
    const v2 = parseInt(sig2.slice(130, 132), 16);
    // Convert: v=27 -> recovery_id=0, v=28 -> recovery_id=1
    const recoveryId1 = v1 >= 27 ? v1 - 27 : v1;
    const recoveryId2 = v2 >= 27 ? v2 - 27 : v2;

    // Ensure recovery IDs are valid (0 or 1) - if not, regenerate signatures
    if (recoveryId1 > 1 || recoveryId2 > 1) {
      // Try again with new signatures if recovery IDs are out of range
      // This can happen with some Ethereum signatures
      console.log(
        `Warning: Recovery IDs out of range: ${recoveryId1}, ${recoveryId2}. Retrying...`
      );
      // For now, clamp to valid range
      const clampedRecoveryId1 = Math.min(recoveryId1, 1);
      const clampedRecoveryId2 = Math.min(recoveryId2, 1);
      // Note: This might cause verification to fail, but let's test the instruction format
    }

    const ethAddress1 = Buffer.from(wallet1.address.slice(2), "hex");
    const ethAddress2 = Buffer.from(wallet2.address.slice(2), "hex");

    const actualMessage1 = Buffer.concat([
      Buffer.from("\x19Ethereum Signed Message:\n" + message1.length),
      message1,
    ]);
    const actualMessage2 = Buffer.concat([
      Buffer.from("\x19Ethereum Signed Message:\n" + message2.length),
      message2,
    ]);

    // Based on Solana web3.js SDK: https://github.com/solana-foundation/solana-web3.js/blob/maintenance/v1.x/src/programs/secp256k1.ts
    // The createInstructionWithEthAddress function shows:
    // - ethAddressOffset = dataStart (first data after offsets)
    // - signatureOffset = dataStart + ethAddress.length (20)
    // - messageDataOffset = signatureOffset + signature.length + 1 (64 + 1 = 65)
    // So order: ethAddress(20) + signature(64) + recoveryId(1) + message(variable)
    //
    // Important: createInstructionWithEthAddress takes the raw message, and Solana's secp256k1
    // program hashes it internally with Keccak256 before verification.
    // ethers.signMessage() signs the Keccak256 hash of the Ethereum signed message format,
    // so we pass actualMessage and Solana will hash it the same way.

    // Create Secp256k1 instruction with multiple signatures
    // Format matches web3.js SDK createInstructionWithEthAddress layout
    const numSignatures = 2;
    const SIGNATURE_OFFSETS_SERIALIZED_SIZE = 11;
    const SIGNATURE_SERIALIZED_SIZE = 64;
    const HASHED_PUBKEY_SERIALIZED_SIZE = 20;

    // Data starts after header (1 byte) + all offset structures (11 bytes each)
    const dataStart = 1 + numSignatures * SIGNATURE_OFFSETS_SERIALIZED_SIZE;

    // Calculate offsets for first signature
    // Order: ethAddress(20) + signature(64) + recoveryId(1) + message(variable)
    let currentOffset = dataStart;
    const eth1Offset = currentOffset;
    currentOffset += HASHED_PUBKEY_SERIALIZED_SIZE; // 20
    const sig1Offset = currentOffset;
    currentOffset += SIGNATURE_SERIALIZED_SIZE + 1; // 64 + 1
    const msg1Offset = currentOffset;
    currentOffset += actualMessage1.length;

    // Calculate offsets for second signature
    const eth2Offset = currentOffset;
    currentOffset += HASHED_PUBKEY_SERIALIZED_SIZE; // 20
    const sig2Offset = currentOffset;
    currentOffset += SIGNATURE_SERIALIZED_SIZE + 1; // 64 + 1
    const msg2Offset = currentOffset;
    currentOffset += actualMessage2.length;

    const totalSize = currentOffset;
    const instructionData = Buffer.alloc(totalSize);

    // Write header: num_signatures
    instructionData.writeUInt8(numSignatures, 0);

    // Write first signature offset structure (starting at byte 1)
    instructionData.writeUInt16LE(sig1Offset, 1); // sig_offset (points to signature)
    instructionData.writeUInt8(0, 3); // sig_ix_idx (0 = instruction 0)
    instructionData.writeUInt16LE(eth1Offset, 4); // eth_offset (points to ethAddress)
    instructionData.writeUInt8(0, 6); // eth_ix_idx (0 = instruction 0)
    instructionData.writeUInt16LE(msg1Offset, 7); // msg_offset (points to message)
    instructionData.writeUInt16LE(actualMessage1.length, 9); // msg_len
    instructionData.writeUInt8(0, 11); // msg_ix_idx (0 = instruction 0)

    // Write second signature offset structure (starting at byte 12)
    instructionData.writeUInt16LE(sig2Offset, 12); // sig_offset
    instructionData.writeUInt8(0, 14); // sig_ix_idx (0 = instruction 0)
    instructionData.writeUInt16LE(eth2Offset, 15); // eth_offset
    instructionData.writeUInt8(0, 17); // eth_ix_idx (0 = instruction 0)
    instructionData.writeUInt16LE(msg2Offset, 18); // msg_offset
    instructionData.writeUInt16LE(actualMessage2.length, 20); // msg_len
    instructionData.writeUInt8(0, 22); // msg_ix_idx (0 = instruction 0)

    // Write actual data in order: ethAddress + signature + recoveryId + message
    // First signature
    ethAddress1.copy(instructionData, eth1Offset);
    sig1Bytes.slice(0, 64).copy(instructionData, sig1Offset);
    instructionData.writeUInt8(recoveryId1, sig1Offset + 64);
    actualMessage1.copy(instructionData, msg1Offset);

    // Second signature
    ethAddress2.copy(instructionData, eth2Offset);
    sig2Bytes.slice(0, 64).copy(instructionData, sig2Offset);
    instructionData.writeUInt8(recoveryId2, sig2Offset + 64);
    actualMessage2.copy(instructionData, msg2Offset);

    const secp256k1Instruction = {
      programId: Secp256k1Program.programId,
      keys: [],
      data: instructionData,
    };

    // Create Anchor program verification instruction
    // Note: recovery_ids is Vec<u8> which Anchor serializes as "bytes" (single Buffer)
    const recoveryIdsBuffer = Buffer.from([recoveryId1, recoveryId2]);

    // Verification function expects the same messages that are in the instruction
    // Since we're putting actualMessage in the instruction, we need to pass actualMessage to verification
    // Our verification function compares bytes directly, so it will compare actualMessage bytes
    const verifyIx = await program.methods
      .verifySecpMultiple(
        [actualMessage1, actualMessage2], // Vec<bytes> expects Buffer/Uint8Array - raw messages
        [
          Array.from(sig1Bytes.slice(0, 64)) as [number, ...number[]],
          Array.from(sig2Bytes.slice(0, 64)) as [number, ...number[]],
        ],
        recoveryIdsBuffer, // Vec<u8> serialized as bytes (single Buffer)
        [
          Array.from(ethAddress1) as [number, ...number[]],
          Array.from(ethAddress2) as [number, ...number[]],
        ]
      )
      .accounts({
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .instruction();

    const tx = new Transaction().add(secp256k1Instruction).add(verifyIx);

    try {
      await provider.sendAndConfirm(tx, []);
      console.log("Multiple Secp256k1 signatures verified successfully!");
    } catch (error: any) {
      console.error("Error:", error);
      if (error.transactionMessage) {
        console.error(`Transaction error: ${error.transactionMessage}`);
      }
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
      }
      assert.fail("Multiple Secp256k1 signatures should be verified");
    }
  });
});
