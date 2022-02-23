package edomata.backend

import FSMDefinition.*

object FSMDefinitionTest {
  final case class State()
  final case class Command()
  final case class Event()
  final case class Notification()
  final case class Rejection()

  type ExampleDomain = HasState[State] And
    HasCommand[Command] And
    HasInternalEvent[Event] And
    HasExternalEvent[Notification] And
    HasRejection[Rejection]

  summon[StateFor[ExampleDomain] =:= State]
  summon[InternalEventFor[ExampleDomain] =:= Event]
  summon[ExternalEventFor[ExampleDomain] =:= Notification]
  summon[RejectionFor[ExampleDomain] =:= Rejection]
  summon[CommandFor[ExampleDomain] =:= Command]

  type AltDomain = (
      HasState[State],
      HasCommand[Command],
      HasInternalEvent[Event],
      HasExternalEvent[Notification],
      HasRejection[Rejection]
  )
  summon[StateFor[AltDomain] =:= State]
  summon[InternalEventFor[AltDomain] =:= Event]
  summon[ExternalEventFor[AltDomain] =:= Notification]
  summon[RejectionFor[AltDomain] =:= Rejection]
  summon[CommandFor[AltDomain] =:= Command]
}
