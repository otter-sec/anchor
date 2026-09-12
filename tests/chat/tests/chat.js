const anchor = require("@anchor-lang/core");
const { assert } = require("chai");
const { PublicKey } = anchor.web3;

describe("chat", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  // Program client handle.
  const program = anchor.workspace.Chat;

  // Chat room account.
  const chatRoom = anchor.web3.Keypair.generate();

  it("Creates a chat room", async () => {
    await program.rpc.createChatRoom("Test Chat", {
      accounts: {
        chatRoom: chatRoom.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      instructions: [
        await program.account.chatRoom.createInstruction(chatRoom),
      ],
      signers: [chatRoom],
    });

    const chat = await program.account.chatRoom.fetch(chatRoom.publicKey);
    const name = new TextDecoder("utf-8").decode(new Uint8Array(chat.name));
    assert.isTrue(name.startsWith("Test Chat")); // [u8; 280] => trailing zeros.
    assert.lengthOf(chat.messages, 33607);
    assert.strictEqual(chat.head.toNumber(), 0);
    assert.strictEqual(chat.tail.toNumber(), 0);
  });

  it("Creates a user", async () => {
    const authority = program.provider.wallet.publicKey;
    const [user, bump] = await PublicKey.findProgramAddress(
      [authority.toBuffer()],
      program.programId
    );
    await program.rpc.createUser("My User", {
      accounts: {
        user,
        authority,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
    });
    const account = await program.account.user.fetch(user);
    assert.strictEqual(account.name, "My User");
    assert.isTrue(account.authority.equals(authority));
  });

  it("Sends messages", async () => {
    const authority = program.provider.wallet.publicKey;
    const user = (
      await PublicKey.findProgramAddress(
        [authority.toBuffer()],
        program.programId
      )
    )[0];

    // Only send a couple messages so the test doesn't take an eternity.
    const numMessages = 10;

    // Generate random message strings.
    const messages = new Array(numMessages).fill("").map((msg) => {
      return (
        Math.random().toString(36).substring(2, 15) +
        Math.random().toString(36).substring(2, 15)
      );
    });

    // Send each message.
    for (let k = 0; k < numMessages; k += 1) {
      console.log("Sending message " + k);
      await program.rpc.sendMessage(messages[k], {
        accounts: {
          user,
          authority,
          chatRoom: chatRoom.publicKey,
        },
      });
    }

    // Check the chat room state is as expected.
    const chat = await program.account.chatRoom.fetch(chatRoom.publicKey);
    const name = new TextDecoder("utf-8").decode(new Uint8Array(chat.name));
    assert.isTrue(name.startsWith("Test Chat")); // [u8; 280] => trailing zeros.
    assert.lengthOf(chat.messages, 33607);
    assert.strictEqual(chat.head.toNumber(), numMessages);
    assert.strictEqual(chat.tail.toNumber(), 0);
    chat.messages.forEach((msg, idx) => {
      if (idx < 10) {
        const data = new TextDecoder("utf-8").decode(new Uint8Array(msg.data));
        console.log("Message", data);
        assert.isTrue(msg.from.equals(user));
        assert.isTrue(data.startsWith(messages[idx]));
      } else {
        assert.strictEqual(
          JSON.stringify(msg.data),
          JSON.stringify(new Array(280).fill(0))
        );
      }
    });
  });
});
