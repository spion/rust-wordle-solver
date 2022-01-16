use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, Error, ErrorKind, Result};
use std::path::Path;

use itertools::Itertools;

enum LetterState {
  Unknown,
  NotPresent,
  Somewhere(HashSet<usize>, HashSet<usize>),
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
enum Mark {
  NotPresent,
  Unpositioned,
  Positioned,
}

type GameState = HashMap<char, LetterState>;

type LetterFrequencies = (usize, HashMap<char, u32>, HashMap<char, u32>);

type DictString = String;

// A state consists of currently wrong letters, currently right letters in the wrong position
// and currently right letters in the right position.

// The state of a letter is Unknown | NotPresent | Positioned(position) | Unpositioned(List(wrongPositions))

// Given these rules, we can construct a filtered word set that only consists of the words
// that have the positioned letters at the exact offset, have the unpositioned letters in places
// any other than the wrong position

// The probability of a word uncovering at least one new letter is 1 - probability_missing_all_letters, or
// 1 - \product from {i = 1..5} (1 - p_i) where p_i is the probability that the letter L is present
// in a word. If a letter repeats, we use its frequency of appearing twice in a word. When it repeats
// the 3rd time, we use the probability of it appearing 3 times and so on
// - this is a simplification because the behavior of repeating letters is a bit different but lets
// - say its good enough heurisitc for now

// From this list we can construct a character frequency map for each (remaining) letter, then using
// that list pick a word that has the most commonly frequent letters with Unknown state. These will
// deliberately have some wrong letters

// We can also pick from a list that falls into tighter constraints. For example we can force the
// use of Positioned letters. We can also force the use of Unpositioned letters that are not in
// the list of wrong positions. After applying those filters we can pick words according to the
// probability mentioned above.

// So a good wordle program will give 2 sets of suggestions - maximizing likelihood of getting a new
// yellow letter VS maximizing likelihood of a correct guess

// Given this state, we need to find the words that either
// - deliberately do a wrong guess but pick
// - match the positioned letters and

fn is_word_valid(state: &GameState, word: &String) -> bool {
  let string_chars_d: Vec<char> = word.chars().collect();
  let string_chars = &string_chars_d;

  for chr in state.keys() {
    match state.get(chr).unwrap_or(&LetterState::Unknown) {
      LetterState::Somewhere(wrong_positions, right_positions) => {
        // If any of the wrong positions for this char are right for this word, bail
        if wrong_positions
          .into_iter()
          .any(|&p| string_chars[p] == *chr)
        {
          return false;
        }

        // If the word doesn't have that many chars of that type, however, also bail.
        // TODO: this condition is naive, we need special cases for something that is present twice
        // vs found not to be present at two previous guesses
        // if string_chars.into_iter().filter(|&c| c == chr).count() != wrong_positions.len() { return false }

        if !string_chars.into_iter().filter(|&c| c == chr).count()
          == wrong_positions.len() + right_positions.len()
        {
          return false;
        }
        if !right_positions
          .into_iter()
          .all(|&p| string_chars[p] == *chr)
        {
          return false;
        }
      }
      LetterState::NotPresent => {
        // If a non-present char is contained, return false
        if string_chars.contains(chr) {
          return false;
        }
      }
      LetterState::Unknown => {}
    }
  }
  return true;
}

fn is_word_relevant(state: &GameState, word: &String) -> bool {
  for chr in word.chars() {
    match state.get(&chr).unwrap_or(&LetterState::Unknown) {
      LetterState::Unknown => {}
      _ => {
        return false;
      }
    }
  }
  return true;
}

fn compute_frequencies(words: &Vec<&DictString>) -> LetterFrequencies {
  let mut character_counts: HashMap<_, _> = ('a'..='z').map(|x| (x, 0)).collect();
  let mut character_counts_twice: HashMap<_, _> = ('a'..='z').map(|x| (x, 0)).collect();

  for word in words {
    let mut individual_counts: HashMap<_, _> = ('a'..='z').map(|x| (x, 0)).collect();
    for chr in word.chars() {
      let individual_count = individual_counts.entry(chr).or_insert(0);
      if individual_count == &0 {
        let count = character_counts.entry(chr).or_insert(0);
        *count += 1;
      } else if individual_count == &1 {
        let count = character_counts_twice.entry(chr).or_insert(0);
        *count += 1;
      }
      *individual_count += 1;
    }
  }

  return (words.len(), character_counts, character_counts_twice);
}

fn compute_guess_probability(word: &DictString, frequencies: &LetterFrequencies) -> f64 {
  let mut product = 1.0;
  let (count, char_freqs, char_freqs_twice) = frequencies;

  let mut used_chars = HashSet::new();

  for chr in word.chars() {
    let char_count_used = if used_chars.contains(&chr) {
      char_freqs_twice.get(&chr).unwrap_or(&0)
    } else {
      char_freqs.get(&chr).unwrap_or(&0)
    };

    let char_probability = 1.0 - (*char_count_used as f64) / (*count as f64);
    used_chars.insert(chr);
    product = product * char_probability;
  }

  return 1.0 - product;
}

fn compute_guess_scores<'a>(
  words: &Vec<&'a DictString>,
  frequencies: LetterFrequencies,
) -> HashMap<&'a DictString, f64> {
  return words
    .into_iter()
    .map(|&x| (x, compute_guess_probability(x, &frequencies)))
    .collect();
}

fn compute_guess_scores_2<'a>(
  words: &Vec<&'a DictString>,
  _frequencies: LetterFrequencies,
) -> HashMap<&'a DictString, f64> {
  return words
    .into_iter()
    .map(|&x| (x, 0.0 - compute_maximal_subset_size(x, &words)))
    .collect();
}

fn compute_bucket(guess: &DictString, word: &DictString) -> Vec<Mark> {
  return guess
    .chars()
    .enumerate()
    .zip(word.chars())
    .map(|((_index, guess_char), word_char)| {
      if guess_char == word_char {
        return Mark::Positioned;
      } else {
        return match word.find(guess_char) {
          None => Mark::NotPresent,
          Some(_) => Mark::Unpositioned,
        };
      }
    })
    .collect();
}

fn compute_maximal_subset_size(guess: &DictString, words: &Vec<&DictString>) -> f64 {
  // (1..=5).map(|n| [Mark::NotPresent, Mark::Positioned, Mark::Unpositioned].map(|p|
  // let all_wordle_outcomes = generate_outcomes()
  // let sets = all_wordle_outcomes()
  // let partitions = words.partition_by(|w| w.match(outcomes))
  // partitions.map(|p| p.count()).max()

  let max_bucket_size = words
    .into_iter()
    .map(|w| {
      let bucket = compute_bucket(guess, w);
      return (bucket.clone(), bucket);
    })
    .into_group_map()
    .into_iter()
    .map(|(_, g)| g.len())
    .max()
    .unwrap_or(0);

  return max_bucket_size as f64;
}

fn reduce_dictionary<'a>(state: &GameState, dict: &Vec<&'a DictString>) -> Vec<&'a DictString> {
  return dict
    .into_iter()
    .filter(|word| is_word_valid(&state, word))
    .map(|&x| x)
    .collect();
}

fn get_suggestions<'a>(
  state: &GameState,
  dict: &Vec<&'a DictString>,
) -> (Vec<(&'a DictString, f64)>, Vec<(&'a DictString, f64)>) {
  let reduced_dict = reduce_dictionary(state, dict);
  let (total, letter_freqs, letter_freqs_twice) = compute_frequencies(&reduced_dict);

  // let guess_probabilities =
  //   compute_guess_scores_2(&dict, &reduced_dict, (total, letter_freqs, letter_freqs_twice));

  // let guess_probabilities =
  //   compute_guess_scores_2(&dict, &reduced_dict, (total, letter_freqs, letter_freqs_twice));

  // guess_probabilities.get(())
  let mut cloned = dict.clone();
  cloned.sort_by(|a, b| {
    let diff =
      guess_probabilities.get(a).unwrap_or(&0.0) - guess_probabilities.get(b).unwrap_or(&0.0);
    if diff > 0.0 {
      Ordering::Less
    } else if diff < 0.0 {
      Ordering::Greater
    } else {
      Ordering::Equal
    }
  });

  let clone_ref = &cloned;

  let top5: Vec<(&DictString, f64)> = clone_ref
    .into_iter()
    .filter(|x| is_word_relevant(&state, x))
    .map(|&x| (x, *guess_probabilities.get(&x).unwrap()))
    .collect();

  let top5valid: Vec<(&DictString, f64)> = clone_ref
    .into_iter()
    .filter(|x| is_word_valid(&state, x))
    .map(|&x| (x, *guess_probabilities.get(&x).unwrap()))
    .collect();

  return (top5, top5valid);
}

fn update_state(state: &mut GameState, word: &str, marks: Vec<Mark>) -> Result<()> {
  let char_marks: Vec<((usize, char), Mark)> =
    word.chars().enumerate().zip(marks.into_iter()).collect();

  for ((offset, chr), mark) in char_marks {
    match (state.entry(chr).or_insert(LetterState::Unknown), mark) {
      (entry_owned @ LetterState::Unknown, Mark::NotPresent) => {
        *entry_owned = LetterState::NotPresent;
      }
      (entry_owned @ LetterState::Unknown, Mark::Unpositioned) => {
        let mut wrong_positions = HashSet::new();
        wrong_positions.insert(offset);
        *entry_owned = LetterState::Somewhere(wrong_positions, HashSet::new());
      }
      (entry_owned @ LetterState::Unknown, Mark::Positioned) => {
        let mut right_positions = HashSet::new();
        right_positions.insert(offset);
        *entry_owned = LetterState::Somewhere(HashSet::new(), right_positions);
      }
      (LetterState::Somewhere(ref mut wrong_positions, _), Mark::Unpositioned) => {
        wrong_positions.insert(offset);
      }
      (LetterState::Somewhere(_, ref mut right_positions), Mark::Positioned) => {
        right_positions.insert(offset);
      }
      _ => return Result::Err(Error::new(ErrorKind::NotFound, "Invalid response")),
    }
  }
  return Result::Ok(());
}

fn read_lines<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>>
where
  P: AsRef<Path>,
{
  let file = File::open(filename)?;
  Ok(io::BufReader::new(file).lines())
}

fn interactive(dictionary: Vec<DictString>, mut state: GameState) {
  let dictionary_ref = dictionary.iter().collect();
  let stdin = io::stdin();

  let (sugg1, sugg2) = get_suggestions(&state, &dictionary_ref);

  println!("Suggestions: {:?} {:?}", sugg1.len(), sugg1.into_iter().take(7).collect::<Vec<_>>());
  println!("Guesses: {:?} {:?}", sugg2.len(), sugg2.into_iter().take(7).collect::<Vec<_>>());

  for line in stdin.lock().lines() {
    let line_content = line.unwrap();
    let word_marks: Vec<&str> = line_content.split(' ').into_iter().collect();
    let used_word = word_marks[0];
    let marks = word_marks[1];

    println!("Got word {} and marks: {}", used_word, marks);

    let update_marks = marks
      .chars()
      .map(|c| match c {
        '-' => Mark::NotPresent,
        '+' => Mark::Unpositioned,
        _ => Mark::Positioned,
      })
      .collect();

    match update_state(&mut state, used_word, update_marks) {
      Result::Ok(()) => {
        println!("State updated");
      }
      Result::Err(_) => {
        println!("Problem updating state");
      }
    }
    let (sugg1, sugg2) = get_suggestions(&state, &dictionary_ref);

    println!("Suggestions: {:?} {:?}", sugg1.len(), sugg1.into_iter().take(7).collect::<Vec<_>>());
    println!("Guesses: {:?} {:?}", sugg2.len(), sugg2.into_iter().take(7).collect::<Vec<_>>());
  }
}

fn main() {
  let dictionary: Vec<DictString> = read_lines("words.txt")
    .unwrap()
    .map(|l| l.unwrap())
    .filter(|l| l.chars().count() == 5 && &l.to_lowercase() == l)
    .collect();

  let state: GameState = HashMap::new();

  interactive(dictionary, state);
}
