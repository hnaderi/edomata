/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.backend

import cats.effect.Concurrent
import cats.effect.Resource
import cats.effect.Sync
import cats.effect.kernel.Async
import cats.effect.std.Semaphore
import cats.implicits._

import scala.collection._

import LRUCache.CacheItem

/** Thread safe, fast, effectful LRU Cache Implemented using HashMap and Deque
  * add and get take O(log(size)) time iterations take O(size) time other
  * operations take constant time
  */
private final class LRUCache[F[_], I, T] private (
    private val values: mutable.Map[I, CacheItem[I, T]],
    private var head: Option[CacheItem[I, T]],
    private var last: Option[CacheItem[I, T]],
    private val sem: Semaphore[F],
    val maxSize: Int
)(using F: Sync[F])
    extends Cache[F, I, T] {
  def size: F[Int] = sem.permit.use(_ => values.size.pure)

  def allValues: Resource[F, Iterable[T]] =
    sem.permit.map(_ => values.values.map(_.value))

  def iterator: Resource[F, Iterator[(I, T)]] =
    sem.permit.map(_ => values.view.mapValues(_.value).iterator)

  def firstValue: F[Option[T]] = sem.permit.use(_ => head.map(_.value).pure)
  def lastValue: F[Option[T]] = sem.permit.use(_ => last.map(_.value).pure)

  def valuesByUsage: Resource[F, Iterator[T]] =
    byUsage.map(_.map(_._2))

  def byUsage: Resource[F, Iterator[(I, T)]] =
    sem.permit.map(_ =>
      LazyList
        .iterate(head)(_.flatMap(_.next))
        .takeWhile(_.isDefined)
        .collect { case Some(v) =>
          (v.key, v.value)
        }
        .iterator
    )
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
