/*
 * Copyright 2021 Beyond Scale Group
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

package golden

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths

/** Generates the golden payload files asserted by the Rust port
  * (`rust/crates/edomata-serde/tests/golden_payloads.rs`): the JSON (and
  * MessagePack) bytes each Scala codec writes for the same sample events.
  *
  * Usage (from the repository root):
  * {{{
  * sbt "examplesJVM/Test/runMain golden.GoldenPayloads rust/tests/golden/payloads"
  * }}}
  */
object GoldenPayloads {
  final case class Money(amount: Long, currency: String)

  enum AccountEvent {
    case Opened(owner: String, initial: Money, tags: List[String])
    case Deposited(amount: Long, note: Option[String], verified: Boolean)
    case Closed
  }

  val samples: List[(String, AccountEvent)] = List(
    "opened" -> AccountEvent
      .Opened("bob", Money(100, "EUR"), List("vip", "eu")),
    "deposited_note" -> AccountEvent.Deposited(42, Some("salary"), true),
    "deposited_no_note" -> AccountEvent.Deposited(7, None, false),
    "closed" -> AccountEvent.Closed
  )

  // --- Circe (generic auto derivation, as documented for edomata users) ----
  object CirceGen {
    import io.circe.generic.auto.*
    import io.circe.syntax.*
    def encode(e: AccountEvent): String = e.asJson.noSpaces
  }

  // --- jsoniter-scala (macro-derived codec with default config) ----------
  object JsoniterGen {
    import com.github.plokhotnyuk.jsoniter_scala.core.*
    import com.github.plokhotnyuk.jsoniter_scala.macros.*
    given JsonValueCodec[AccountEvent] = JsonCodecMaker.make
    def encode(e: AccountEvent): String = writeToString(e)
    // `JsoniterCodec.msgpack` actually writes JSON bytes with `writeToArray`.
    def encodeMsgpack(e: AccountEvent): Array[Byte] = writeToArray(e)
  }

  // --- uPickle (default ReadWriter derivation) ------------------------------
  object UpickleGen {
    import upickle.default.*
    given ReadWriter[Money] = macroRW
    given ReadWriter[AccountEvent] = ReadWriter.derived
    def encode(e: AccountEvent): String = write(e)
    // Real MessagePack: documented as unreadable by the Rust port.
    def encodeMsgpack(e: AccountEvent): Array[Byte] = writeBinary(e)
  }

  def main(args: Array[String]): Unit = {
    val dir = Paths.get(args.headOption.getOrElse("rust/tests/golden/payloads"))
    Files.createDirectories(dir)
    def write(name: String, bytes: Array[Byte]): Unit = {
      Files.write(dir.resolve(name), bytes)
      println(s"wrote ${dir.resolve(name)}")
    }
    def utf8(s: String) = s.getBytes(StandardCharsets.UTF_8)
    for ((name, event) <- samples) {
      write(s"circe_$name.json", utf8(CirceGen.encode(event)))
      write(s"jsoniter_$name.json", utf8(JsoniterGen.encode(event)))
      write(s"jsoniter_msgpack_$name.bin", JsoniterGen.encodeMsgpack(event))
      write(s"upickle_$name.json", utf8(UpickleGen.encode(event)))
      write(s"upickle_msgpack_$name.bin", UpickleGen.encodeMsgpack(event))
    }
  }
}
