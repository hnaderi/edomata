/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    "introduction",
    {
      type: "category",
      label: "About",
      items: ["about/design-goals", "about/features"],
    },
    {
      type: "category",
      label: "Tutorials",
      items: [
        "tutorials/getting-started",
        "tutorials/eventsourcing",
        "tutorials/cqrs",
        "tutorials/backends",
        "tutorials/processes",
        "tutorials/saas",
        "tutorials/migrations",
      ],
    },
    {
      type: "category",
      label: "Principles",
      items: ["principles/index", "principles/definitions"],
    },
    {
      type: "category",
      label: "Backends",
      items: ["backends/skunk", "backends/doobie"],
    },
    {
      type: "category",
      label: "Other",
      items: ["other/modules", "other/faq"],
    },
  ],
};

module.exports = sidebars;
