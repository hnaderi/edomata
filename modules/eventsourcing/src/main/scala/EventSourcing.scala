package edomata.eventsourcing

import cats.data.ValidatedNec

type Fold[S, E, R] = E => S => ValidatedNec[R, S]
