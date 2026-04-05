// Publish to GitHub Packages when PUBLISH_TO_GITHUB is set.
// Used by .github/workflows/publish-ghp.yml on every merge to main or v* tag push.
//
// Versioning (handled by sbt-dynver via sbt-typelevel):
//   - Tagged commit (v0.12.1)  → "0.12.1"       (release)
//   - Untagged main commit     → "0.12.0+7-abc" (snapshot-like, unique per commit)
//
// Consumers add to build.sbt:
//   resolvers += "BSG GitHub Packages" at "https://maven.pkg.github.com/beyond-scale-group/edomata"
//   credentials += Credentials("GitHub Package Registry", "maven.pkg.github.com", "_", sys.env("GITHUB_TOKEN"))
inThisBuild(
  if (sys.env.contains("PUBLISH_TO_GITHUB")) {
    val ghOwner = "beyond-scale-group"
    val ghRepo = "edomata"
    List(
      publishTo := Some(
        "GitHub Packages" at s"https://maven.pkg.github.com/$ghOwner/$ghRepo"
      ),
      credentials += Credentials(
        "GitHub Package Registry",
        "maven.pkg.github.com",
        "_",
        sys.env.getOrElse("GITHUB_TOKEN", "")
      ),
      // Disable gpg signing — GitHub Packages does not require it
      gpgWarnOnFailure := true
    )
  } else Nil
)
