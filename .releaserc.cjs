module.exports = {
  branches: ['main'],
  plugins: [
    '@semantic-release/commit-analyzer',
    '@semantic-release/release-notes-generator',
    ['@semantic-release/npm', { npmPublish: false }],
    ['@semantic-release/exec', {
      prepareCmd: 'bash scripts/bump-cargo-version.sh ${nextRelease.version}',
    }],
    ['@semantic-release/changelog', { changelogFile: 'changelog.md' }],
    ['@semantic-release/git', {
      assets: ['changelog.md', 'package.json', 'package-lock.json', 'Cargo.toml', 'Cargo.lock'],
      message: 'chore(release): ${nextRelease.version} [skip ci]\n\n${nextRelease.notes}',
    }],
    ['@semantic-release/github', { failComment: false, failTitle: false }],
  ],
};
