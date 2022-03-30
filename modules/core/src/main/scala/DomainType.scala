package edomata.core

sealed trait DomainType

object DomainType {
  sealed trait HasCommand[C] extends DomainType
  sealed trait HasModelTypes[S, E, R] extends DomainType
  sealed trait HasNotification[N] extends DomainType
  sealed trait HasMetadata[M] extends DomainType
  type HasModel[M <: Model[?, ?, ?]] <: DomainType = M match {
    case Model[s, e, r] => HasModelTypes[s, e, r]
  }
}
