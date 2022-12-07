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

import cats.effect.IO
import cats.implicits.*
import munit.CatsEffectSuite

import LRUCacheSuite.*

class LRUCacheSuite extends CatsEffectSuite {
  test("must store what's added") {
    for {
      c <- emptyCache
      _ <- c.add(1, 2).assertEquals(None)
      _ <- c.get(1).assertEquals(Some(2))
      _ <- c.iterator.use(_.toList.pure).assertEquals(List((1, 2)))
    } yield ()
  }

  test("must evict least recently used item when reaches its max size") {
    for {
      c <- newCache
      items = c.byUsage.use(_.toList.pure)
      _ <- c.add(4, 8).assertEquals(Some((1, 2)))
      i1 <- items
      _ <- c.get(1).assertEquals(None)
      i2 <- items
    } yield {
      assertEquals(i1, i2)
      assertEquals(i1, (2 to 4).reverse.toList.map(i => (i, i * 2)))
    }
  }

  test("must keep recently used items fresh") {
    for {
      c <- newCache
      items = c.valuesByUsage.use(_.toList.pure)
      _ <- items.assertEquals(List(6, 4, 2))
      _ <- c.get(2).assertEquals(Some(4))
      _ <- items.assertEquals(List(4, 6, 2))
      _ <- c.get(3).assertEquals(Some(6))
      _ <- items.assertEquals(List(6, 4, 2))
      _ <- c.get(1).assertEquals(Some(2))
      _ <- items.assertEquals(List(2, 6, 4))
    } yield ()
  }

  test("must replace if predicate matches") {
    for {
      c <- newCache
      _ <- c.replace(1, 3)(_ == 2)
      _ <- c.get(1).assertEquals(Some(3))
    } yield ()
  }

  test("must not replace if predicate does not match") {
    for {
      c <- newCache
      _ <- c.replace(1, 3)(_ == 4)
      _ <- c.get(1).assertEquals(Some(2))
    } yield ()
  }

  test("must add when replacing and non existing key value") {
    for {
      c <- newCache
      _ <- c.replace(4, 5)(_ == 4)
      _ <- c.get(4).assertEquals(Some(5))
    } yield ()
  }

}

object LRUCacheSuite {
  private val emptyCache = LRUCache[IO, Int, Int](3)
  private val newCache =
    emptyCache.flatTap(c => (1 to 3).toList.traverse(i => c.add(i, i * 2)))
}
