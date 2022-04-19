package edomata.backend

import munit.FunSuite

class PGNamespaceSuite extends FunSuite {
  test("Macro constructor") {
    PGNamespace("a")
    assert(
      compileErrors("PGNamespace(\"\")").contains("Name: \"\" does not match")
    )
  }
  test("Length limit") {
    assertEquals(
      PGNamespace.fromString("a" * 64),
      Left("Name is too long: 64 (max allowed is 63)")
    )
  }
}
