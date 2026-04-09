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

package edomata.java

import java.util.function.{Function => JFunction}

sealed abstract class JEither[+L, +R] {
  def isLeft: Boolean
  def isRight: Boolean

  def fold[L2 >: L, R2 >: R, C](
      onLeft: JFunction[L2, C],
      onRight: JFunction[R2, C]
  ): C =
    this match {
      case JEither.Left(value)  => onLeft.apply(value)
      case JEither.Right(value) => onRight.apply(value)
    }

  def map[R2 >: R, B](f: JFunction[R2, B]): JEither[L, B] =
    this match {
      case JEither.Left(value)  => JEither.Left(value)
      case JEither.Right(value) => JEither.Right(f.apply(value))
    }

  def getRight: R = this match {
    case JEither.Right(value) => value
    case JEither.Left(_)      =>
      throw new java.util.NoSuchElementException("JEither.getRight on Left")
  }

  def getLeft: L = this match {
    case JEither.Left(value) => value
    case JEither.Right(_)    =>
      throw new java.util.NoSuchElementException("JEither.getLeft on Right")
  }
}

object JEither {
  final case class Left[+L](value: L) extends JEither[L, Nothing] {
    def isLeft: Boolean = true
    def isRight: Boolean = false
  }

  final case class Right[+R](value: R) extends JEither[Nothing, R] {
    def isLeft: Boolean = false
    def isRight: Boolean = true
  }

  def left[L, R](value: L): JEither[L, R] = Left(value)
  def right[L, R](value: R): JEither[L, R] = Right(value)
}
