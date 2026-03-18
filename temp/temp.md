## Create Github Templates

I want to create issue templates and pr template that followw industry best practices.

- For tags I like the scheme for example, but not limited to this feat:{feature-name}

Make this based on industry best practices and also fits the projects purpose.

## Issue Creation Template - Multiple

Go through the @docs/plans/theming plan and lets create github issues to track the feature implementation.

- Group tasks together that logically fit (no need for an issue per task)
- Make sure the issues are properly labeled
- Create the necessary labels as needed for the issue
- Labels must include feat:{feature-name} additive of any other labels
- Make sure the issues are properly prioritized
- Make them detailed and include which tasks they cover in title and/or body.
- The task must include the numbers, such as 1.1, 1.2, 2.1 - 2.4, etc.
- This will be read by AI agents so make it consumable and actionable for them to execute
- No relative http(s) links, as those break inside the issue pointing to /issue/{#}/{link}
- Only relative directory links to files
  - such as [@docs/plans/{feature-name}/parallel-plan.md](file:///home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/plans/{feature-name}/parallel-plan.md)

## Issue Creation Template - Single

Go through the @docs/plans/progressive-disclosure plan and lets create a github issue to track the feature implementation.

- Make sure the issue is properly labeled
- Labels must include feat:{feature-name} additive of any other labels
- Make sure the issue is properly prioritized
- Make it detailed and include tasks details the body
- The task must include the numbers, such as 1.1, 1.2, 2.1 - 2.4, etc.
- This will be read by AI agents so make it consumable and actionable for them to execute
- No relative http(s) links, as those break inside the issue pointing to /issue/{#}/{link}
- Only relative directory links to files
  - such as [@docs/plans/{feature-name}/parallel-plan.md](file:///home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/plans/{feature-name}/parallel-plan.md)

## PR Review Correction

Let's validate and fix suggestions issues 19-24

- file: [@pr-146-review.md](file:///home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/pr-reviews/pr-146-review.md)
- validate before implementing
- run targeted tests
- update doc when complete ([@pr-146-review.md](file:///home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/pr-reviews/pr-146-review.md))
- when confirmed fix, commit progress
