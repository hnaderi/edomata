package edomata.eventsourcing
import fs2.Stream

trait SnapshotStore[F[_], I, S] {
  def get(id: I): F[Option[S]]
  def put(id: I, state: S): F[Unit]
}
