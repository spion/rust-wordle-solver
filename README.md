# rust-wordle-solver

Solves wordle optimally by means of set subdivision

# Building and running

You should probably use the release build, as the debug build may be too slow

```
cargo build --release
./target/release/wordle-solver
```

# Usage

The solver will output two types of words: candidate suggestions and candidate guesses,
along with their scores. After doing that, it will wait on your input describing the word
you picked and the Wordle outcome you got:

Example exchange (optimal candidate openers censored to avoid spoilers):

```
> Suggestions: 5758 [("<word>", -355.0), ("<word>", -366.0), ("<word>", -377.0), ("<word>", -393.0), ("<word>", -393.0), ("<word>", -395.0), ("<word>", -395.0)]
> Guesses: 5758 [("<word>", -355.0), ("<word>", -366.0), ("<word>", -377.0), ("<word>", -393.0), ("<word>", -393.0), ("<word>", -395.0), ("<word>", -395.0)]
<word> --+--
> Got word <word> and marks: --+--
> Suggestions: 5758 [("croon", -15.0), ("crony", -15.0), ("grout", -15.0), ("loony", -16.0), ("trunk", -17.0), ("croup", -17.0), ("troop", -18.0)]
> Guesses: 125 [("croon", -15.0), ("crony", -15.0), ("grout", -15.0), ("trunk", -17.0), ("croup", -17.0), ("troop", -18.0), ("grunt", -18.0)]
croon -oo--
> Got word croon and marks: -oo--
> Suggestions: 5758 [("plate", -3.0), ("youth", -3.0), ("depth", -3.0), ("unity", -3.0), ("dusty", -3.0), ("flute", -3.0), ("rusty", -3.0)]
> Guesses: 15 [("trout", -5.0), ("wroth", -5.0), ("group", -6.0), ("proud", -6.0), ("prowl", -6.0), ("troll", -6.0), ("grout", -6.0)]
```

Suggestions are words that don't necessarily conform to the constraints that Wordle has
presented so far, but would reduce the set of possible words very well. Guesses are words that
do conform to the constraints - they are also sorted by how well they will subdivide the
remaining possible guesses

```
USAGE:
    wordle-solver [OPTIONS]

OPTIONS:
    -d, --dict <DICT>            Path to the word dictionary to use [default: words.txt]
    -g, --gambling <GAMBLING>    Use a gambling strategy (instead of a best-average case default)
        --guesses <GUESSES>      Path to a reduced guess dictionary to use
    -h, --help                   Print help information
    -p, --pessimistic            Use the worst case strategy (instead of best average case default).
                                 Good against Absurdle
    -V, --version                Print version information
    -w, --word <WORD>            Disables interactive mode and replays a game to guess the specified
                                 word
```

# How does it work?

For each possible guess, we subdivide the set of words into different subsets based on what colors wordle would give us for that word. Then we score the words based on the subset sizes

- By default, we score by the average amount of information a guess is most likely to yield
- By passing `--pessimistic` you can get the best-worst-case guess (useful for playing [Absurdle][1])
- By passing `--gambling` you can get a percentile-case of your chosing (0 is worst case, 0.5 is
  median guess)

 The worst case scenario is illustrated in a [JamBoard presentation][2]


[1]: https://qntm.org/files/absurdle/absurdle.html
[2]: https://jamboard.google.com/d/1weQUvRyrVqaYsPRa_qhH-NycKwW3TSUqa46_CBsYGSk/viewer?f=0