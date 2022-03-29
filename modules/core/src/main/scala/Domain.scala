package edomata.core

opaque type Domain = Unit

object Domain {
  opaque type HasCommand[C] <: Domain = Domain
  opaque type HasModel[S, E, R] <: Domain = Domain
  opaque type HasNotification[N] <: Domain = Domain
  opaque type HasMetadata[M[_] <: CommandMetadata[_]] <: Domain = Domain

  extension (domain: Domain) {
    def withMetadata[M[_] <: CommandMetadata[_]]: domain.type & HasMetadata[M] =
      ().asInstanceOf
    def withCommand[C]: domain.type & HasCommand[C] =
      ().asInstanceOf
    def withModel[M <: Model[?, ?, ?]]: domain.type &
      HasModel[M, Model.EventFrom[M], Model.RejectionFrom[M]] =
      ().asInstanceOf
    def withNotification[N]: domain.type & HasNotification[N] =
      ().asInstanceOf
  }

  def default: HasMetadata[CommandMessage] = ()
  def withMetadata[M[_] <: CommandMetadata[_]]: HasMetadata[M] = ()
  def withCommand[C]: HasCommand[C] = ()
  def withModel[M <: Model[?, ?, ?]]
      : HasModel[M, Model.EventFrom[M], Model.RejectionFrom[M]] = ()
  def withNotification[N]: HasNotification[N] = ()
}
