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
```

Suggestions are words that don't necessarily conform to the constraints that Wordle has
presented so far, but would reduce the set of possible words very well. Guesses are words that
do conform to the constraints - they are also sorted by how well they will subdivide the
remaining possible guesses

# How does it work?

For each possible guess, we subdivide the set of words into different subsets based on what colors wordle would give us for that word. Then we score the words based on the largest subset size. We aim to pick a word where the largest subset is the smallest.

This is illustrated in the following jamboard: https://jamboard.google.com/d/1weQUvRyrVqaYsPRa_qhH-NycKwW3TSUqa46_CBsYGSk/viewer?f=0
