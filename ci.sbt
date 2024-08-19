val PrimaryJava = JavaSpec.temurin("8")
val LTSJava = JavaSpec.temurin("17")

inThisBuild(
  List(
    tlSiteJavaVersion := LTSJava,
    githubWorkflowJavaVersions := Seq(PrimaryJava, LTSJava),
    githubWorkflowBuildPreamble ++= dockerComposeUp,
    githubWorkflowJobSetup ~= {
      _.filterNot(_.name.exists(_.matches("(Download|Setup) Java .+")))
    },
    githubWorkflowJobSetup += WorkflowStep.Use(
      UseRef.Public("cachix", "install-nix-action", "v17"),
      name = Some("Install Nix")
    ),
    githubWorkflowSbtCommand := "nix develop .#${{ matrix.java }} -c sbt",
    // This job is used as a sign that all build jobs have been successful and is used by mergify
    githubWorkflowAddedJobs += WorkflowJob(
      id = "post-build",
      name = "post build",
      needs = List("build"),
      steps = List(
        WorkflowStep.Run(
          commands = List("echo success!"),
          name = Some("post build")
        )
      ),
      scalas = Nil,
      javas = Nil
    )
  )
)

lazy val dockerComposeUp = Seq(
  WorkflowStep.Run(
    commands = List("docker compose up -d"),
    name = Some("Start up Postgres")
  )
)
