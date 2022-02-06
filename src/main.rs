use clap::Parser;
use itertools::Itertools;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Result};
use std::path::Path;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum Mark {
  NotPresent = 0,
  WrongPosition = 1,
  RightPosition = 2,
}

type DictString = String;

#[derive(Debug, PartialEq, Clone, Copy)]
enum Strategy {
  WorstCase,
  Gambling(f64),
  Average,
}

const SHOWN_GUESSES: usize = 10;

fn compute_guess_scores<'a>(
  words_all: &Vec<&'a DictString>,
  words_reduced: &Vec<&'a DictString>,
  strategy: Strategy,
) -> HashMap<&'a DictString, f64> {
  return words_all
    .par_iter()
    .map(|&x| (x, compute_information_value(x, &words_reduced, strategy)))
    .collect();
}

fn compute_bucket_sizes(guess: &DictString, words: &Vec<&DictString>) -> Vec<usize> {
  words
    .into_iter()
    .map(|w| (compute_bucket(guess, w), w))
    .into_group_map()
    .into_iter()
    .map(|(_, g)| g.len())
    .collect::<Vec<_>>()
}

fn compute_information_value(
  guess: &DictString,
  words: &Vec<&DictString>,
  strategy: Strategy,
) -> f64 {
  let mut bucket_sizes = compute_bucket_sizes(guess, words);

  match strategy {
    Strategy::WorstCase => {
      let worst_case_count = bucket_sizes.into_iter().max().unwrap_or(0) as f64;
      return (words.len() as f64 / worst_case_count).log2();
    }
    Strategy::Average => {
      let information_amount: f64 = bucket_sizes
        .into_iter()
        .map(|sz| {
          let guess_probability = sz as f64 / words.len() as f64;
          let log_info = (1.0 / guess_probability).log2();
          return guess_probability * log_info;
        })
        .sum();

      return information_amount;
    }
    Strategy::Gambling(gambling_factor) => {
      bucket_sizes.sort_by(|a, b| {
        if a > b {
          Ordering::Less
        } else if a < b {
          Ordering::Greater
        } else {
          Ordering::Equal
        }
      });

      let mut total_size = 0;

      for size in bucket_sizes {
        total_size += size;
        let new_gambling = total_size as f64 / words.len() as f64;
        if new_gambling > gambling_factor {
          return (words.len() as f64 / size as f64).log2();
        }
      }

      return 0.0;
    }
  }
}

/// This function tries to faithfully reproduce the same algorithm as found
/// in the original Wordle. Letters from the word get "used up" first by their presence
/// at the exact same position (i.e. "green" marks). Then if a letter appears multiple times in
/// the guess but at the wrong position, it will start using up the same letter in the word, to
/// ensure that if there is just a single occurence of the guessed letter in the word, only the
/// first occurrence in the guess gets marked "yellow" (wrong position)
///
// #[memoize]
fn compute_bucket(guess: &DictString, word: &DictString) -> Vec<Mark> {
  let mut used = vec![false; word.len()];
  let mut result = vec![Mark::NotPresent; word.len()];

  for ((index, guess_char), word_char) in guess.chars().enumerate().zip(word.chars()) {
    if word_char == guess_char {
      used[index] = true;
      result[index] = Mark::RightPosition;
    }
  }

  for (guess_index, guess_char) in guess.chars().enumerate() {
    for (word_index, word_char) in word.chars().enumerate() {
      if result[guess_index] == Mark::RightPosition {
        continue;
      }
      if used[word_index] {
        continue;
      }
      if word_index == guess_index {
        continue;
      }

      if guess_char == word_char {
        used[word_index] = true;
        result[guess_index] = Mark::WrongPosition;
        break;
      }
    }
  }

  return result;
}

fn reduce_dictionary<'a>(
  guess: &DictString,
  marks: &Vec<Mark>,
  dict: &Vec<&'a DictString>,
) -> Vec<&'a DictString> {
  return dict
    .into_par_iter()
    .filter(|word| &compute_bucket(guess, word) == marks)
    .map(|&x| x)
    .collect();
}

fn get_suggestions<'a>(
  dict: &Vec<&'a DictString>,
  reduced_dict: &Vec<&'a DictString>,
  strategy: Strategy,
) -> (Vec<(&'a DictString, f64)>, Vec<(&'a DictString, f64)>) {
  let scores = compute_guess_scores(&dict, &reduced_dict, strategy);

  let score_criteria = |a: &&DictString, b: &&DictString| {
    let diff = scores.get(a).unwrap_or(&0.0) - scores.get(b).unwrap_or(&0.0);
    if diff < 0.0 {
      Ordering::Greater
    } else if diff > 0.0 {
      Ordering::Less
    } else {
      Ordering::Equal
    }
  };

  let mut cloned = dict.clone();
  cloned.par_sort_by(score_criteria);

  let top5sugg = (&cloned)
    .into_iter()
    .map(|&x| (x, *scores.get(&x).unwrap()))
    .collect();

  let mut reduced_cloned = reduced_dict.clone();
  reduced_cloned.par_sort_by(score_criteria);

  let top5guess = (&reduced_cloned)
    .into_iter()
    .map(|&x| (x, *scores.get(&x).unwrap()))
    .collect();

  return (top5sugg, top5guess);
}

fn read_lines<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>>
where
  P: AsRef<Path>,
{
  let file = File::open(filename)?;
  Ok(io::BufReader::new(file).lines())
}

fn interactive(
  dictionary: Vec<DictString>,
  reducing_dictionary: Vec<DictString>,
  strategy: Strategy,
) {
  let dictionary_ref: Vec<&DictString> = dictionary.iter().collect();
  let mut reducing_dictionary_ref = reducing_dictionary.iter().collect();

  let stdin = io::stdin();

  let (sugg1, sugg2) = get_suggestions(&dictionary_ref, &reducing_dictionary_ref, strategy);

  println!(
    "Suggestions: {:?} {:?}",
    sugg1.len(),
    sugg1.into_iter().take(SHOWN_GUESSES).collect::<Vec<_>>()
  );
  println!(
    "Guesses: {:?} {:?}",
    sugg2.len(),
    sugg2.into_iter().take(SHOWN_GUESSES).collect::<Vec<_>>()
  );

  for line in stdin.lock().lines() {
    let line_content = line.unwrap();
    let word_marks: Vec<&str> = line_content.split(' ').into_iter().collect();
    let used_word = String::from(word_marks[0]);
    let marks = word_marks[1];

    println!("Got word {} and marks: {}", used_word, marks);

    let update_marks: Vec<Mark> = marks
      .chars()
      .map(|c| match c {
        '-' => Mark::NotPresent,
        '+' => Mark::WrongPosition,
        _ => Mark::RightPosition,
      })
      .collect();

    reducing_dictionary_ref =
      reduce_dictionary(&used_word, &update_marks, &reducing_dictionary_ref);

    let (ref sugg1, ref sugg2) =
      get_suggestions(&dictionary_ref, &reducing_dictionary_ref, strategy);

    println!(
      "Suggestions: {:?} {:?}",
      sugg1.len(),
      sugg1.into_iter().take(SHOWN_GUESSES).collect::<Vec<_>>()
    );
    println!(
      "Guesses: {:?} {:?}",
      sugg2.len(),
      sugg2.into_iter().take(SHOWN_GUESSES).collect::<Vec<_>>()
    );

    let (sug_word, sug_score) = sugg1[0];
    let (guess_word, guess_score) = sugg2[0];

    let attempt_word = if sug_score >= guess_score + 0.005 {
      sug_word
    } else {
      guess_word
    };

    println!("Suggest you try {:?}", attempt_word);
  }
}

fn play_word(
  word: String,
  dictionary: Vec<DictString>,
  reducing_dictionary: Vec<DictString>,
  strategy: Strategy,
) {
  let dict_ref: Vec<&DictString> = dictionary.iter().collect();
  let mut reducing_dict_ref: Vec<&DictString> = reducing_dictionary.iter().collect();

  let mut tries = 0;
  loop {
    let (ref suggestions, ref guesses) = get_suggestions(&dict_ref, &reducing_dict_ref, strategy);

    if guesses.len() == 0 {
      println!("Stumped, cannot figure it out");
      break;
    } else if guesses.len() == 1 {
      tries += 1;
      println!(
        "Got it on try {:?}! The answer is: {:?}",
        tries, guesses[0].0
      );
      break;
    } else {
      println!(
        "Suggestions: {:?} {:?}",
        suggestions.len(),
        suggestions
          .into_iter()
          .take(SHOWN_GUESSES)
          .collect::<Vec<_>>()
      );
      println!(
        "Guesses: {:?} {:?}",
        guesses.len(),
        guesses.into_iter().take(SHOWN_GUESSES).collect::<Vec<_>>()
      );

      let (sug_word, sug_score) = suggestions[0];
      let (guess_word, guess_score) = guesses[0];

      // let remaining_guess_bits = (1.0 / guesses.len() as f64).log2();
      // let after_suggestion_bits = remaining_guess_bits - sug_score;
      // let after_guess_bits = remaining_guess_bits - guess_score;

      let attempt_word = if sug_score >= guess_score + 0.005 {
        sug_word
      } else {
        guess_word
      };

      tries += 1;

      println!("Try {:?}, word {:?}", tries, attempt_word);

      let outcome = compute_bucket(attempt_word, &word);

      if outcome == vec![Mark::RightPosition; 5] {
        println!("Actually guessed it!");
        break;
      } else {
        println!("Outcome: {:?}", outcome);

        reducing_dict_ref = reduce_dictionary(&attempt_word, &outcome, &reducing_dict_ref);
      }
    }
  }
}

fn read_dict(file: &str) -> Vec<DictString> {
  read_lines(file)
    .unwrap()
    .map(|l| l.unwrap())
    .filter(|l| l.chars().count() == 5 && &l.to_lowercase() == l)
    .collect()
}

/// A wordle solver
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  /// Path to the word dictionary to use
  #[clap(short, long, default_value = "words.txt")]
  dict: String,

  /// Path to a reduced guess dictionary to use
  #[clap(long)]
  guesses: Option<String>,

  /// Use a gambling strategy (instead of a best-average case default)
  #[clap(short, long)]
  gambling: Option<f64>,

  /// Use the worst case strategy (instead of best average case default). Good against Absurdle
  #[clap(short, long)]
  pessimistic: bool,

  /// Disables interactive mode and replays a game to guess the specified word
  #[clap(short, long)]
  word: Option<String>,
}

fn main() {
  let args = Args::parse();

  let dictionary: Vec<DictString> = read_dict(&args.dict);

  let dictionary_reduced: Vec<DictString> = match args.guesses {
    None => dictionary.clone(),
    Some(file) => read_dict(&file),
  };

  let strategy = match (args.gambling, args.pessimistic) {
    (None, false) => Strategy::Average,
    (None, true) => Strategy::WorstCase,
    (Some(factor), false) => Strategy::Gambling(factor),
    (_, _) => {
      panic!("Wrong set of options")
    }
  };

  match args.word {
    None => {
      return interactive(dictionary, dictionary_reduced, strategy);
    }
    Some(word) => {
      return play_word(word, dictionary, dictionary_reduced, strategy);
    }
  }
}
