// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

(() => {
  "use strict";

  const quiz = document.querySelector(".check-yourself");
  if (!quiz) {
    return;
  }

  const result = quiz.querySelector(".quiz-result");

  quiz.querySelector(".check-answers").addEventListener("click", () => {
    let allCorrect = true;

    quiz.querySelectorAll("fieldset[data-answer]").forEach((question) => {
      const selected = question.querySelector("input:checked");
      const correct = selected?.value === question.dataset.answer;
      const incorrect = selected !== null && !correct;
      const feedback = question.querySelector(".question-feedback");

      question.classList.toggle("is-correct", correct);
      question.classList.toggle("is-incorrect", incorrect);
      allCorrect = allCorrect && correct;

      if (!selected) {
        feedback.textContent =
          "Choose an answer. Telepathy support remains unimplemented.";
      } else if (correct) {
        feedback.textContent = "Correct.";
      } else {
        feedback.textContent = question.dataset.explanation;
      }
    });

    result.textContent = allCorrect
      ? "Correct. The YAML did not win this round."
      : "Not yet. Review the marked answer and try again.";
  });
})();
