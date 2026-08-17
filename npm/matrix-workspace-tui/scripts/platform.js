'use strict';

/**
 * Map Node's platform/arch to the release asset platform name.
 * Asset names on GitHub releases: matrix-workspace-tui-<platform>.
 */
function getPlatform() {
  const mapping = {
    'linux-x64': 'linux-x64',
    'linux-arm64': 'linux-arm64',
    'darwin-x64': 'darwin-x64',
    'darwin-arm64': 'darwin-arm64',
  };
  const name = mapping[`${process.platform}-${process.arch}`];
  if (!name) {
    throw new Error(
      `Unsupported platform ${process.platform}-${process.arch}. ` +
        'Supported: linux-x64, linux-arm64, darwin-x64, darwin-arm64.',
    );
  }
  return { name, binaryName: `matrix-workspace-tui-${name}` };
}

module.exports = { getPlatform };
