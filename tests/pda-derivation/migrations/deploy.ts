// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@anchor-lang/core");

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  // Add your deploy script here.
  async function deployAsync(exampleString: string): Promise<void> {
    return new Promise((resolve) => {
      setTimeout(() => {
        console.log(exampleString);
        resolve();
      }, 2000);
    });
  }

  await deployAsync("Typescript migration example complete.");
};
