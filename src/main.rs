use std::iter::FromIterator;
use clap::Parser;
use itertools::Itertools;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Result};
use std::mem::transmute;
use std::path::Path;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum Mark {
  NotPresent = 0,
  WrongPosition = 1,
  RightPosition = 2,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
struct VecMark(u8);

impl VecMark {

  pub fn of(np: Mark) -> Self {
    VecMark::new(np, np, np, np, np)
  }
  pub fn new(m1: Mark, m2: Mark, m3: Mark, m4: Mark, m5: Mark) -> Self {
    return Self(m1 as u8 + m2 as u8 * 3 + m3 as u8 * 9 + m4 as u8 * 27 + m5 as u8 * 81);
  }

  pub fn get(&self, ix: usize) -> Mark {
    let res = self.0 / 3_i32.pow(ix as u32) as u8 % 3;
    unsafe { transmute::<u8, Mark>(res) }
  }

  pub fn set(&mut self, ix: usize, m:Mark) {
    let old_val = self.get(ix);
    self.0 = self.0 - old_val as u8 + (3_i32.pow(ix as u32) * (m as i32)) as u8;
  }
}
impl FromIterator<Mark> for VecMark {

  fn from_iter<I: IntoIterator<Item=Mark>>(iter: I) -> Self {
    let mut v = VecMark::of(Mark::NotPresent);
    let mut ix = 0;
    for item in iter {
      v.set(ix, item);
      ix += 1;
    }

    return v;
  }
}

type DictString = String;

fn compute_guess_scores<'a>(
  words_all: &Vec<&'a DictString>,
  words_reduced: &Vec<&'a DictString>,
  gambling_factor: f64,
) -> HashMap<&'a DictString, f64> {
  return words_all
    .into_iter()
    .map(|&x| {
      (
        x,
        0.0 - compute_percentile_subset_sz(x, &words_reduced, gambling_factor),
      )
    })
    .collect();
}

fn compute_percentile_subset_sz(
  guess: &DictString,
  words: &Vec<&DictString>,
  gambling_factor: f64,
) -> f64 {
  let buckets = words
    .into_iter()
    .map(|w| (compute_bucket(guess, w), 1))
    .into_group_map()
    .into_iter()
    .map(|(_, g)| g.len());

  if gambling_factor <= 0.01 {
    return buckets.max().unwrap_or(0) as f64;
  } else {
    let mut bucket_sizes: Vec<usize> = buckets.collect();
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
        return size as f64;
      }
    }

    return 0.0;
  }
}

/// This function tries to faithfully reproduce the same algorithm as found
/// in the original Wordle. Letters from the word get "used up" first by their presence
/// at the exact same position (i.e. "green" marks). Then if a letter appears multiple times in
/// the guess but at the wrong position, it will start using up the same letter in the word, to
/// ensure that if there is just a single occurence of the guessed letter in the word, only the
/// first occurrence in the guess gets marked "yellow" (wrong position)
///
fn compute_bucket(guess: &DictString, word: &DictString) -> VecMark {
  let mut used = vec![false; word.len()];
  let mut result = VecMark::of(Mark::NotPresent);  //vec![Mark::NotPresent; word.len()];

  for ((index, guess_char), word_char) in guess.chars().enumerate().zip(word.chars()) {
    if word_char == guess_char {
      used[index] = true;
      result.set(index, Mark::RightPosition);
    }
  }

  for (guess_index, guess_char) in guess.chars().enumerate() {
    for (word_index, word_char) in word.chars().enumerate() {
      if result.get(guess_index) == Mark::RightPosition {
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
        result.set(guess_index, Mark::WrongPosition);
        break;
      }
    }
  }

  return result;
}

fn reduce_dictionary<'a>(
  guess: &DictString,
  marks: VecMark,
  dict: &Vec<&'a DictString>,
) -> Vec<&'a DictString> {
  return dict
    .into_iter()
    .filter(|word| compute_bucket(guess, word) == marks)
    .map(|&x| x)
    .collect();
}

fn get_suggestions<'a>(
  dict: &Vec<&'a DictString>,
  reduced_dict: &Vec<&'a DictString>,
  gambling_factor: f64,
) -> (Vec<(&'a DictString, f64)>, Vec<(&'a DictString, f64)>) {
  let scores = compute_guess_scores(&dict, &reduced_dict, gambling_factor);

  let score_criteria = |a: &&DictString, b: &&DictString| {
    let diff = scores.get(a).unwrap_or(&0.0) - scores.get(b).unwrap_or(&0.0);
    if diff > 0.0 {
      Ordering::Less
    } else if diff < 0.0 {
      Ordering::Greater
    } else {
      Ordering::Equal
    }
  };

  let mut cloned = dict.clone();
  cloned.sort_by(score_criteria);

  let top5sugg = (&cloned)
    .into_iter()
    .map(|&x| (x, *scores.get(&x).unwrap()))
    .collect();

  let mut reduced_cloned = reduced_dict.clone();
  reduced_cloned.sort_by(score_criteria);

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
  gambling_factor: f64,
) {
  let dictionary_ref: Vec<&DictString> = dictionary.iter().collect();
  let mut reducing_dictionary_ref = reducing_dictionary.iter().collect();

  let stdin = io::stdin();

  let (sugg1, sugg2) = get_suggestions(&dictionary_ref, &reducing_dictionary_ref, gambling_factor);

  println!(
    "Suggestions: {:?} {:?}",
    sugg1.len(),
    sugg1.into_iter().take(7).collect::<Vec<_>>()
  );
  println!(
    "Guesses: {:?} {:?}",
    sugg2.len(),
    sugg2.into_iter().take(7).collect::<Vec<_>>()
  );

  for line in stdin.lock().lines() {
    let line_content = line.unwrap();
    let word_marks: Vec<&str> = line_content.split(' ').into_iter().collect();
    let used_word = String::from(word_marks[0]);
    let marks = word_marks[1];

    println!("Got word {} and marks: {}", used_word, marks);

    let update_marks: VecMark = marks
      .chars()
      .map(|c| match c {
        '-' => Mark::NotPresent,
        '+' => Mark::WrongPosition,
        _ => Mark::RightPosition,
      })
      .collect();

    reducing_dictionary_ref =
      reduce_dictionary(&used_word, update_marks, &reducing_dictionary_ref);

    let (ref sugg1, ref sugg2) =
      get_suggestions(&dictionary_ref, &reducing_dictionary_ref, gambling_factor);

    println!(
      "Suggestions: {:?} {:?}",
      sugg1.len(),
      sugg1.into_iter().take(10).collect::<Vec<_>>()
    );
    println!(
      "Guesses: {:?} {:?}",
      sugg2.len(),
      sugg2.into_iter().take(10).collect::<Vec<_>>()
    );

    let (sug_word, sug_score) = sugg1[0];
    let (guess_word, guess_score) = sugg2[0];

    let attempt_word = if sug_score >= guess_score + 0.1 {
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
  gambling_factor: f64,
) {
  let dict_ref: Vec<&DictString> = dictionary.iter().collect();
  let mut reducing_dict_ref: Vec<&DictString> = reducing_dictionary.iter().collect();

  let mut tries = 0;
  loop {
    let (ref suggestions, ref guesses) =
      get_suggestions(&dict_ref, &reducing_dict_ref, gambling_factor);

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
        suggestions.into_iter().take(10).collect::<Vec<_>>()
      );
      println!(
        "Guesses: {:?} {:?}",
        guesses.len(),
        guesses.into_iter().take(10).collect::<Vec<_>>()
      );

      let (sug_word, sug_score) = suggestions[0];
      let (guess_word, guess_score) = guesses[0];

      let attempt_word = if sug_score >= guess_score + 0.1 {
        sug_word
      } else {
        guess_word
      };

      tries += 1;

      println!("Try {:?}, word {:?}", tries, attempt_word);

      let outcome = compute_bucket(attempt_word, &word);

      if outcome == VecMark::of(Mark::RightPosition) {
        println!("Actually guessed it!");
        break;
      } else {
        println!("Outcome: {:?}", outcome);

        reducing_dict_ref = reduce_dictionary(&attempt_word, outcome, &reducing_dict_ref);
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
  #[clap(short, long)]
  guesses: Option<String>,

  /// Gambling factor. How optimistic should the bot be while playing (max=1.0)
  #[clap(short, long, default_value_t = 0.0)]
  gambling: f64,

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

  match args.word {
    None => {
      interactive(dictionary, dictionary_reduced, args.gambling);
    }
    Some(word) => {
      play_word(word, dictionary, dictionary_reduced, args.gambling);
    }
  }
}
