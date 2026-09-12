const anchor = require("@anchor-lang/core");

describe("tictactoe", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Tictactoe;
  let dashboard = anchor.web3.Keypair.generate();
  let game = anchor.web3.Keypair.generate();
  let player_o = anchor.web3.Keypair.generate();

  it("Initialize Dashboard", async () => {
    const tx = await program.rpc.initializeDashboard({
      accounts: {
        authority: program.provider.wallet.publicKey,
        dashboard: dashboard.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [dashboard],
      instructions: [
        await program.account.dashboard.createInstruction(dashboard),
      ],
    });

    console.log("transaction: ", tx);
  });

  it("Initialize Game", async () => {
    const tx = await program.rpc.initialize({
      accounts: {
        playerX: program.provider.wallet.publicKey,
        dashboard: dashboard.publicKey,
        game: game.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [game],
      instructions: [await program.account.game.createInstruction(game)],
    });

    console.log("transaction: ", tx);
  });

  it("Player O joins", async () => {
    const tx = await program.rpc.playerJoin({
      accounts: {
        playerO: player_o.publicKey,
        game: game.publicKey,
      },
      signers: [player_o],
    });

    console.log("transaction: ", tx);
  });

  it("Player x plays", async () => {
    const tx = await program.rpc.playerMove(1, 0, {
      accounts: {
        player: program.provider.wallet.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction: ", tx);
  });

  it("Player o plays", async () => {
    const tx = await program.rpc.playerMove(2, 1, {
      accounts: {
        player: player_o.publicKey,
        game: game.publicKey,
      },
      signers: [player_o],
    });
    console.log("transaction: ", tx);
  });

  it("Player x plays", async () => {
    const tx = await program.rpc.playerMove(1, 3, {
      accounts: {
        player: program.provider.wallet.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction: ", tx);
  });

  it("Player o plays", async () => {
    const tx = await program.rpc.playerMove(2, 6, {
      accounts: {
        player: player_o.publicKey,
        game: game.publicKey,
      },
      signers: [player_o],
    });
    console.log("transaction: ", tx);
  });

  it("Player x plays", async () => {
    const tx = await program.rpc.playerMove(1, 2, {
      accounts: {
        player: program.provider.wallet.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction: ", tx);
  });

  it("Player o plays", async () => {
    const tx = await program.rpc.playerMove(2, 4, {
      accounts: {
        player: player_o.publicKey,
        game: game.publicKey,
      },
      signers: [player_o],
    });
    console.log("transaction: ", tx);
  });

  it("Player x plays", async () => {
    const tx = await program.rpc.playerMove(1, 5, {
      accounts: {
        player: program.provider.wallet.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction: ", tx);
  });

  it("Player o plays", async () => {
    const tx = await program.rpc.playerMove(2, 8, {
      accounts: {
        player: player_o.publicKey,
        game: game.publicKey,
      },
      signers: [player_o],
    });
    console.log("transaction: ", tx);
  });

  it("Player x plays", async () => {
    const tx = await program.rpc.playerMove(1, 7, {
      accounts: {
        player: program.provider.wallet.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction: ", tx);
  });

  it("Status", async () => {
    const tx = await program.rpc.status({
      accounts: {
        dashboard: dashboard.publicKey,
        game: game.publicKey,
      },
    });

    console.log("transaction: ", tx);
  });
});
