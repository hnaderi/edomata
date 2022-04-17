package edomata.backend

sealed trait BackendError extends Throwable
object BackendError {
  case object VersionConflict extends Throwable(""), BackendError
  case object MaxRetryExceeded
      extends Throwable("Maximum number of retries exceeded!"),
        BackendError
  final case class UnknownError(underlying: Throwable)
      extends Throwable("Unknown error!", underlying),
        BackendError
  final case class PersistenceError(msg: String)
      extends Throwable(msg),
        BackendError
}
