package edfsm.eventsourcing

import cats.effect.Concurrent
import cats.effect.kernel.Ref
import cats.implicits.*

object InMemSnapshotStore {
  def apply[F[_], I <: String, S](using
      F: Concurrent[F]
  ): F[SnapshotStore[F, I, S]] =
    for {
      st <- Ref[F].of(Map.empty[String, S])
    } yield new SnapshotStore[F, I, S] {
      def get(id: I): F[Option[S]] = st.get.map(_.get(id))
      def put(id: I, state: S): F[Unit] = st.update(_.updated(id, state))
    }
}
