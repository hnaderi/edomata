package edomata.eventsourcing

import scala.collection._

import cats.effect.Concurrent
import cats.effect.Sync
import cats.effect.std.Semaphore
import cats.implicits._
import LRUCache.CacheItem
import cats.effect.kernel.Async

/** Thread safe, fast, effectful LRU Cache Implemented using HashMap and Deque
  * add and get take O(log(size)) time iterations take O(size) time other
  * operations take constant time
  */
@SuppressWarnings(
  Array(
    "org.wartremover.warts.All"
  )
)
final class LRUCache[F[_], I, T] private (
    private val values: mutable.Map[I, CacheItem[I, T]],
    private var head: Option[CacheItem[I, T]],
    private var last: Option[CacheItem[I, T]],
    private val sem: Semaphore[F],
    val maxSize: Int
)(using F: Sync[F]) {
  def size: Int = values.size

  def allValues: Iterable[T] = values.values.map(_.value)

  def iterator: Iterator[(I, T)] = values.view.mapValues(_.value).iterator

  def firstValue: Option[T] = head.map(_.value)
  def lastValue: Option[T] = last.map(_.value)

  def valuesByUsage: Iterator[T] =
    LazyList
      .iterate(head)(_.flatMap(_.next))
      .takeWhile(_.isDefined)
      .collect { case Some(v) =>
        v.value
      }
      .iterator

  def add(key: I, value: T): F[Option[(I, T)]] =
    sem.permit.use { _ =>
      F.delay {
        values.get(key) match {
          case Some(existing) =>
            existing.value = value
            makeHead(existing)
          case None =>
            val newItem = CacheItem(key, value, None, head)
            values.update(key, newItem)
            makeHead(newItem)
        }

        evict()
      }
    }

  def get(key: I): F[Option[T]] =
    sem.permit.use { _ =>
      F.delay(
        values.get(key) match {
          case Some(existing) =>
            makeHead(existing)
            Some(existing.value)
          case None => None
        }
      )
    }

  private def evict() =
    if (values.size > maxSize) {
      val lastValue = last.map(_.key).flatMap(values.remove)
      val beforeLast = last.flatMap(_.prev)
      beforeLast.foreach(_.next = None)
      last = beforeLast
      lastValue.map(v => (v.key, v.value))
    } else None

  private def makeHead(item: CacheItem[I, T]): Unit = {
    if (head.contains(item)) return
    if (head.exists(_.next.isEmpty)) last = head
    if (last.contains(item)) last = item.prev

    // joining
    item.prev.foreach(_.next = item.next)
    item.next.foreach(_.prev = item.prev)

    // inserting
    item.prev = None
    item.next = head
    head.foreach(_.prev = Some(item))

    // replacing
    head = Some(item)
  }
}

@SuppressWarnings(
  Array(
    "org.wartremover.warts.MutableDataStructures",
    "org.wartremover.warts.Var"
  )
)
object LRUCache {
  final case class CacheItem[I, T](
      val key: I,
      var value: T,
      var prev: Option[CacheItem[I, T]],
      var next: Option[CacheItem[I, T]]
  )

  def apply[F[_]: Async, Id, State](size: Int): F[LRUCache[F, Id, State]] =
    for {
      sem <- Semaphore[F](1)
    } yield new LRUCache[F, Id, State](mutable.Map.empty, None, None, sem, size)
}
