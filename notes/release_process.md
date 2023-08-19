# Release Process
Insh and inshd share version numbers.

## Insh/Inshd
To create a new release for insh/inshd perform the following:
- Bump the version in `insh/Cargo.toml` and `inshd/Cargo.toml` (the versions should match).
- Rebuild (so that `Cargo.lock` is updated with the new version number).
- Update `CHANGELOG.md`.
- Commit the above changes and merge to master.
- Checkout the commit on the master branch that the release will be made from.
- Run `git tag -a v<version>` where `<version>` is replaced with the new version number.
    - The title of the message should be "Version <version>".
    - The body of the message should be the bullet points of the CHANGELOG for the version.
- Run `git push origin v<version>`.
- Update the `latest` tag.
    - Run `git tag -f latest`.
    - Run `git push --force origin latest`.
- Draft a new release on GitHub.
    - Use the tag created for the new version. 
    - The release title should be "Version <version>".
    - The body should be the bullet points of the CHANGELOG for the version.
- Publish the new GitHub release.
