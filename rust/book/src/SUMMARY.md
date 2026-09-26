# Summary

[Introduction](introduction.md)

# About

- [Design goals](about/design-goals.md)
- [Features](about/features.md)

# Tutorials

- [Getting started](tutorials/getting-started.md)
- [Event sourcing](tutorials/eventsourcing.md)
- [CQRS style](tutorials/cqrs.md)
- [Running](tutorials/backends.md)
- [Processes](tutorials/processes.md)
- [SaaS multi-tenant module](tutorials/saas.md)
- [Event migrations](tutorials/migrations.md)

# Principles

- [Principles](principles/index.md)
- [Definitions](principles/definitions.md)

# Backends and integrations

- [PostgreSQL (sqlx)](backends/postgres.md)
- [Simple API](backends/simple-api.md)
- [Distributing events with Kafka / RabbitMQ](backends/brokers.md)

# Other

- [Crates](other/modules.md)
- [Migration guide for Scala and Java users](other/migration-guide.md)
- [Porting map](other/porting.md)
- [FAQ](other/faq.md)

# Design decisions

- [Architecture decision records](design/index.md)
  - [0001 Effects and async runtime](design/adr-0001.md)
  - [0002 Timestamps with chrono](design/adr-0002.md)
  - [0003 Edomaton representation](design/adr-0003.md)
  - [0004 Response and RaiseError](design/adr-0004.md)
  - [0005 Backend abstractions](design/adr-0005.md)
  - [0006 PostgreSQL naming and golden DDL](design/adr-0006.md)
  - [0007 Codecs and the jsonb wire format](design/adr-0007.md)
  - [0008 The sqlx driver](design/adr-0008.md)
  - [0009 Test kit and SaaS](design/adr-0009.md)
  - [0010 The simple facade](design/adr-0010.md)
  - [0011 E2E, examples and the cross-language test](design/adr-0011.md)
  - [0012 Broker distribution](design/adr-0012.md)
  - [0013 Documentation](design/adr-0013.md)
