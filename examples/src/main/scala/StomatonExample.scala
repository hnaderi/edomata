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

package edomata.core

import cats.*
import cats.data.*
import cats.effect.IO
import cats.implicits.*
import edomata.backend.Backend
import edomata.skunk.*
import edomata.syntax.all.*
import io.circe.Codec

object StomatonExample {
  enum Foo {
    case Empty
    case Started(value: Int)

    def start(initial: Int): EitherNec[String, Foo] = this match {
      case Empty      => Right(Started(initial))
      case Started(_) => "Cannot start started foo!".leftNec
    }
  }

  object Foo extends CQRSModel[Foo, String] {
    override def initial: Foo = Empty
  }

  object FooService extends Foo.Service[Int, Int] {
    def apply(): PureApp[Unit] = Stomaton.unit

    import dsl.*
    def apply2(): PureApp[Foo] = for {
      _ <- pure(1)
      ns <- decideS(_.start(1))
      _ <- Stomaton.unit
    } yield ns
    //
    // def apply3(): PureApp[Foo] =
    //   pure(1).flatMap(modifyS(_.start(1)))
  }

  val res = FooService().run(???, Foo.Empty)

  val out = res.result match {
    case Right((newState, _)) => ???
    case Left(errs)           => ???
  }

  given Codec[Foo] = ???
  given Codec[Int] = ???
  val driver: edomata.backend.cqrs.StorageDriver[IO, Codec, IO] = ???
  val backend = Backend.builder(FooService).use(driver).build

  val handler: SkunkHandler[IO][Int] = SkunkHandler {
    case i if i < 5 => _ => IO.println(i)
    case other      => _ => IO.println(s"$other is higher than 5")
  }

  given BackendCodec[Foo] = ???
  given BackendCodec[Int] = ???
  val backend2 =
    Backend
      .builder(FooService)
      .use(SkunkCQRSDriver[IO]("example", ???))
      .withEventHandler(handler)
      .build

  backend.use { b =>
    val srv = b.compile(FooService().liftTo[IO])

    srv(???)
  }
}
