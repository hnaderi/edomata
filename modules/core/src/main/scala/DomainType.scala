package edomata.core

import scala.annotation.showAsInfix

sealed trait DomainType

object DomainType {
  sealed trait HasCommand[C] extends DomainType
  sealed trait HasModelTypes[S, E, R] extends DomainType
  sealed trait HasNotification[N] extends DomainType
  sealed trait HasMetadata[M] extends DomainType
  type HasModel[M <: Model[?, ?, ?]] <: DomainType = M match {
    case Model[s, e, r] => HasModelTypes[s, e, r]
  }

  @showAsInfix
  type And[B, A <: DomainType] <: Tuple = B match {
    case DomainType => A *: B *: EmptyTuple
    case Tuple      => A *: B
  }

  type CommandFor[D] = D match {
    case HasCommand[c] *: t => c
    case h *: t             => CommandFor[t]
  }

  type MetadataFor[D] = D match {
    case HasMetadata[m] *: t => m
    case h *: t              => MetadataFor[t]
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
  }

  type ModelFor[D] = D match {
    case HasModelTypes[s, e, r] *: t => Model[s, e, r]
    case h *: t                      => ModelFor[t]
  }

}
