const anchor = require("@anchor-lang/core");

describe("tictactoe", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.tictactoe;

  const dashboard = anchor.web3.Keypair.generate();
  const game = anchor.web3.Keypair.generate();
  const playerO = anchor.web3.Keypair.generate();
  const playerX = program.provider.wallet;

  it("Initialize Dashboard", async () => {
    const tx = await program.rpc.initializeDashboard({
      accounts: {
        authority: playerX.publicKey,
        dashboard: dashboard.publicKey,
      },
      signers: [dashboard],
      instructions: [
        await program.account.dashboard.createInstruction(dashboard),
      ],
    });
    console.log("transaction:", tx);
  });

  it("Initialize Game", async () => {
    const tx = await program.rpc.initialize({
      accounts: {
        playerX: playerX.publicKey,
        dashboard: dashboard.publicKey,
        game: game.publicKey,
      },
      signers: [game],
      instructions: [await program.account.game.createInstruction(game)],
    });
    console.log("transaction:", tx);
  });

  it("Player O joins", async () => {
    const tx = await program.rpc.playerJoin({
      accounts: {
        playerO: playerO.publicKey,
        game: game.publicKey,
      },
      signers: [playerO],
    });
    console.log("transaction:", tx);
  });

  it("Player X plays", async () => {
    const tx = await program.rpc.playerMove(1, 0, {
      accounts: {
        player: playerX.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });

  it("Player O plays", async () => {
    const tx = await program.rpc.playerMove(2, 1, {
      accounts: {
        player: playerO.publicKey,
        game: game.publicKey,
      },
      signers: [playerO],
    });
    console.log("transaction:", tx);
  });

  it("Player X plays", async () => {
    const tx = await program.rpc.playerMove(1, 3, {
      accounts: {
        player: playerX.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });

  it("Player O plays", async () => {
    const tx = await program.rpc.playerMove(2, 6, {
      accounts: {
        player: playerO.publicKey,
        game: game.publicKey,
      },
      signers: [playerO],
    });
    console.log("transaction:", tx);
  });

  it("Player X plays", async () => {
    const tx = await program.rpc.playerMove(1, 2, {
      accounts: {
        player: playerX.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });

  it("Player O plays", async () => {
    const tx = await program.rpc.playerMove(2, 4, {
      accounts: {
        player: playerO.publicKey,
        game: game.publicKey,
      },
      signers: [playerO],
    });
    console.log("transaction:", tx);
  });

  it("Player X plays", async () => {
    const tx = await program.rpc.playerMove(1, 5, {
      accounts: {
        player: playerX.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });

  it("Player O plays", async () => {
    const tx = await program.rpc.playerMove(2, 8, {
      accounts: {
        player: playerO.publicKey,
        game: game.publicKey,
      },
      signers: [playerO],
    });
    console.log("transaction:", tx);
  });

  it("Player X plays", async () => {
    const tx = await program.rpc.playerMove(1, 7, {
      accounts: {
        player: playerX.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });

  it("Status", async () => {
    const tx = await program.rpc.status({
      accounts: {
        dashboard: dashboard.publicKey,
        game: game.publicKey,
      },
    });
    console.log("transaction:", tx);
  });
});
