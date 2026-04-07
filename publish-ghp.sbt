// Publish to GitHub Packages when PUBLISH_TO_GITHUB is set.
// Used by .github/workflows/publish-ghp.yml on v* tag push.
//
// How it works:
//   - publishTo and gpgWarnOnFailure are set at PROJECT scope in build.sbt
//     (via ghpPublishSettings in module()) to override sbt-typelevel's
//     TypelevelSonatypePlugin and sbt-gpg which set these per-project.
//   - credentials are set at ThisBuild scope in build.sbt (works fine — credentials accumulate).
//
// Versioning (handled by sbt-dynver via sbt-typelevel):
//   - Tagged commit (v0.12.7)  → "0.12.7"       (release)
//   - Untagged main commit     → "0.12.0+7-abc" (snapshot-like, unique per commit)
//
// Consumers add to build.sbt:
//   resolvers += "BSG GitHub Packages" at "https://maven.pkg.github.com/beyond-scale-group/edomata"
//   credentials += Credentials("GitHub Package Registry", "maven.pkg.github.com", "_", sys.env("GITHUB_TOKEN"))
