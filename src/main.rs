use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Result};
use std::path::Path;

use itertools::Itertools;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum Mark {
  NotPresent,
  WrongPosition,
  RightPosition,
}

type DictString = String;

fn compute_guess_scores<'a>(
  words_all: &Vec<&'a DictString>,
  words_reduced: &Vec<&'a DictString>,
) -> HashMap<&'a DictString, f64> {
  return words_all
    .into_iter()
    .map(|&x| (x, 0.0 - compute_maximal_subset_size(x, &words_reduced)))
    .collect();
}

fn compute_maximal_subset_size(guess: &DictString, words: &Vec<&DictString>) -> f64 {
  let max_bucket_size = words
    .into_iter()
    .map(|w| (compute_bucket(guess, w), 1))
    .into_group_map()
    .into_iter()
    .map(|(_, g)| g.len())
    .max()
    .unwrap_or(0);

  return max_bucket_size as f64;
}

/// This function tries to faithfully reproduce the same algorithm as found
/// in the original Wordle. Letters from the word get "used up" first by their presence
/// at the exact same position (i.e. "green" marks). Then if a letter appears multiple times in
/// the guess but at the wrong position, it will start using up the same letter in the word, to
/// ensure that if there is just a single occurence of the guessed letter in the word, only the
/// first occurrence in the guess gets marked "yellow" (wrong position)
///
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
      if guess_char == word_char && word_index != guess_index && !used[word_index] {
        used[word_index] = true;
        result[guess_index] = Mark::WrongPosition;
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
    .into_iter()
    .filter(|word| &compute_bucket(guess, word) == marks)
    .map(|&x| x)
    .collect();
}

fn get_suggestions<'a>(
  dict: &Vec<&'a DictString>,
  reduced_dict: &Vec<&'a DictString>,
) -> (Vec<(&'a DictString, f64)>, Vec<(&'a DictString, f64)>) {
  let guess_scores = compute_guess_scores(&dict, &reduced_dict);

  let mut cloned = dict.clone();

  let score_criteria = |a: &&DictString, b: &&DictString| {
    let diff = guess_scores.get(a).unwrap_or(&0.0) - guess_scores.get(b).unwrap_or(&0.0);
    if diff > 0.0 {
      Ordering::Less
    } else if diff < 0.0 {
      Ordering::Greater
    } else {
      Ordering::Equal
    }
  };

  cloned.sort_by(score_criteria);

  let mut reduced_cloned = reduced_dict.clone();

  reduced_cloned.sort_by(score_criteria);

  let clone_ref = &cloned;
  let reduced_cloned_ref = &reduced_cloned;

  let top5: Vec<(&DictString, f64)> = clone_ref
    .into_iter()
    .map(|&x| (x, *guess_scores.get(&x).unwrap()))
    .collect();

  let top5valid: Vec<(&DictString, f64)> = reduced_cloned_ref
    .into_iter()
    .map(|&x| (x, *guess_scores.get(&x).unwrap()))
    .collect();

  return (top5, top5valid);
}

fn read_lines<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>>
where
  P: AsRef<Path>,
{
  let file = File::open(filename)?;
  Ok(io::BufReader::new(file).lines())
}

fn interactive(dictionary: Vec<DictString>) {
  let dictionary_ref: Vec<&DictString> = dictionary.iter().collect();
  let mut reducing_dictionary_ref = dictionary_ref.clone();

  let stdin = io::stdin();

  let (sugg1, sugg2) = get_suggestions(&dictionary_ref, &reducing_dictionary_ref);

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

    let (sugg1, sugg2) = get_suggestions(&dictionary_ref, &reducing_dictionary_ref);

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
  }
}

fn main() {
  let dictionary: Vec<DictString> = read_lines("words.txt")
    .unwrap()
    .map(|l| l.unwrap())
    .filter(|l| l.chars().count() == 5 && &l.to_lowercase() == l)
    .collect();

  interactive(dictionary);
}
