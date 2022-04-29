# FAQ

## Should I use it?
Like everything in engineering it depends; you should first consider answering the following questions, and your answers will lead you to deciding.

### Are you sure event sourcing/CQRS is solution to your problem?
This is the most common problem developers face when want to use any tools or frameworks in this context;
if you are not sure, or don't have experience using those techniques,
chances are really high that you would not be happy to use these techniques for a real project for the first time, as these techniques will introduce a lot of different challenges and accidental complexity if not used in the right context;
so you should consider using other techniques which require other tools.  
On the other hand, if you know what you are doing and have experience using these techniques, or are trying to learn something new without commitments of a real project that's perfect! I'm pretty sure you'll find this library at least interesting.

### Are you/your team familiar with Cats and other typelevel libraries/ or haskell?
While this library is designed to be as general as possible, but ideas and communities are vital parts of designing and developing software and if you feel new, you should get yourself familiar. there are a lot of docs/tutorials/talks/books that can help you dive in, and typelevel community is one of the most friendly and informed ones out there, and you'll get help if you ask.  
If you are familiar, then Welcome! :)

## Why yet another lib?
I discussed it in [rationale](../introduction.md#rationale).

## Can I use a different backend?
Absolutely! backends are just simple interpreters for you programs, you can implement them as easily as creating a function that takes an app and returns an effect, as simple as that.  
I also might add new backends in the future if requested, feel free to open issues or pull requests!

## What the point of this library? I can create my own data structures?
Exactly! that's how simple it is. I started to think about creating this library after a lot of times doing exactly that!
While it's not rocket science to create a monadic data type, it's not what you want to focus on when you are writing business logic;
and creating effect stacks of `IRWST`, `ReaderT[[tt]=>>WriterT[[t]=>>EitherT[F, NonEmptyChain[R], t], List[E], tt], C]` won't be much appealing either!  
and having that much flexibility leads to unnecessary cognitive load, which we can use on solving real business problems.  
If you are familiar with how this library is built and its ideas, it won't be more than a few opinionated, tailored data types for creating programs, which is exactly the goal of this library.

>> Note that I've not checked that huge type with compiler, but you'll get the point :))
