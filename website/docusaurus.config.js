// @ts-check

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "Edomata",
  tagline: "Event-driven automatons for Scala, Scala.js, and Scala Native",
  favicon: "img/icon.png",

  url: "https://beyond-scale-group.github.io",
  baseUrl: "/edomata/",

  organizationName: "beyond-scale-group",
  projectName: "edomata",

  onBrokenLinks: "throw",

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "warn",
    },
  },

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: require.resolve("./sidebars.js"),
          editUrl:
            "https://github.com/beyond-scale-group/edomata/tree/main/docs/",
        },
        theme: {
          customCss: require.resolve("./src/css/custom.css"),
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      image: "img/icon.png",
      colorMode: {
        defaultMode: "dark",
        disableSwitch: false,
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: "Edomata",
        logo: {
          alt: "Edomata Logo",
          src: "img/icon.png",
        },
        items: [
          {
            type: "docSidebar",
            sidebarId: "docs",
            position: "left",
            label: "Documentation",
          },
          {
            href: "https://github.com/beyond-scale-group/edomata",
            label: "GitHub",
            position: "right",
          },
        ],
      },
      footer: {
        style: "dark",
        links: [
          {
            title: "Documentation",
            items: [
              { label: "Introduction", to: "/docs/introduction" },
              { label: "Getting Started", to: "/docs/tutorials/getting-started" },
              { label: "Principles", to: "/docs/principles/" },
            ],
          },
          {
            title: "Backends",
            items: [
              { label: "Skunk", to: "/docs/backends/skunk" },
              { label: "Doobie", to: "/docs/backends/doobie" },
            ],
          },
          {
            title: "Ecosystem",
            items: [
              { label: "Cats", href: "https://typelevel.org/cats/" },
              { label: "Cats Effect", href: "https://typelevel.org/cats-effect/" },
              { label: "FS2", href: "https://fs2.io/" },
              { label: "GitHub", href: "https://github.com/beyond-scale-group/edomata" },
            ],
          },
        ],
        copyright: `Copyright ${new Date().getFullYear()} Beyond Scale Group. Built with Docusaurus.`,
      },
      prism: {
        theme: require("prism-react-renderer").themes.github,
        darkTheme: require("prism-react-renderer").themes.dracula,
        additionalLanguages: ["java", "sql", "bash"],
      },
    }),
};

module.exports = config;
