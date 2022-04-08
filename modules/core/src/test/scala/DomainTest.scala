package edomata.core

import Domain.*

object DomainTest {
  final case class Event()
  final case class Rejection()
  abstract class State extends Model[State, Event, Rejection]
  final case class Command()
  final case class Notification()

  type ExampleDomain = HasModel[State] And
    HasCommand[Command] And
    HasMetadata[Map[String, String]] And HasNotification[Notification]

  summon[ModelFor[ExampleDomain] =:= Model[State, Event, Rejection]]
  summon[StateFor[ExampleDomain] =:= State]
  summon[EventFor[ExampleDomain] =:= Event]
  summon[NotificationFor[ExampleDomain] =:= Notification]
  summon[RejectionFor[ExampleDomain] =:= Rejection]
  summon[CommandFor[ExampleDomain] =:= Command]

  type AltDomain = (
      HasModel[State],
      HasCommand[Command],
      HasMetadata[Map[String, String]],
      HasNotification[Notification]
  )

  summon[ModelFor[AltDomain] =:= Model[State, Event, Rejection]]
  summon[StateFor[AltDomain] =:= State]
  summon[EventFor[AltDomain] =:= Event]
  summon[NotificationFor[AltDomain] =:= Notification]
  summon[RejectionFor[AltDomain] =:= Rejection]
  summon[CommandFor[AltDomain] =:= Command]
}
