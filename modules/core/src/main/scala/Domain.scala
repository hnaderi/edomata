package edomata.core

import scala.annotation.showAsInfix

sealed trait Domain

object Domain {
  sealed trait HasCommand[C] extends Domain
  sealed trait HasModelTypes[S, E, R] extends Domain
  sealed trait HasNotification[N] extends Domain

  type HasModel[M <: Model[?, ?, ?]] <: Domain = M match {
    case Model[s, e, r] => HasModelTypes[s, e, r]
  }

  @showAsInfix
  type And[B, A <: Domain] <: Tuple = B match {
    case Domain => A *: B *: EmptyTuple
    case Tuple  => A *: B
  }

  type CommandFor[D] = D match {
    case HasCommand[c] *: t => c
    case h *: t             => CommandFor[t]
  }

  type NotificationFor[D] = D match {
    case HasNotification[e] *: t => e
    case h *: t                  => NotificationFor[t]
  }

  type StateFor[D] = D match {
    case HasModelTypes[s, e, r] *: t => s
    case h *: t                      => StateFor[t]
  }

  type EventFor[D] = D match {
    case HasModelTypes[s, e, r] *: t => e
    case h *: t                      => EventFor[t]
  }

  type RejectionFor[D] = D match {
    case HasModelTypes[s, e, r] *: t => r
    case h *: t                      => RejectionFor[t]
    case EmptyTuple                  => String
  }

  type ModelFor[D] = D match {
    case HasModelTypes[s, e, r] *: t => Model[s, e, r]
    case h *: t                      => ModelFor[t]
  }

}
