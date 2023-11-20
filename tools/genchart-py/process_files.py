from collections import Counter
from datetime import datetime
from typing import Optional, Dict


class CommitData:
    def __init__(self):
        self.all_authors = {}
        self.commit_dates = []
        self.all_prs_per_author = {}
        self.status_distribution = Counter()
        self.merge_time = []
        self.approved_prs = 0
        self.self_approved_prs = 0

    @staticmethod
    def get_login(value: Optional[Dict]) -> Optional[str]:
        return value.get("login") if value else None

    def process_json_data(self, json_data: Dict) -> None:
        for commit in json_data:
            self.process_commit(commit)

    def process_commit(self, commit: Dict) -> None:
        # Process authors
        authors = commit.get("authors", {}).get("nodes", [])
        for author in authors:
            author_name = author.get("name")
            if author_name:
                self.all_authors[author_name] = self.all_authors.get(author_name, 0) + 1

        # Process commit date
        commit_date = commit.get("committedDate")
        if commit_date:
            self.commit_dates.append(datetime.fromisoformat(commit_date).date())

        # Process associated pull requests
        prs = commit.get("associatedPullRequests", {}).get("nodes", [])
        prs = filter(lambda pr: isinstance(pr, dict), prs)
        for pr in prs:
            self.process_pr(pr)

    def process_pr(self, pr: Dict) -> None:
        pr_author = self.get_login(pr.get("author"))
        merged_by = self.get_login(pr.get("mergedBy"))
        status = pr.get("state")
        if not pr_author or not merged_by or not status:
            return

        self.all_prs_per_author[pr_author] = (
            self.all_prs_per_author.get(pr_author, 0) + 1
        )
        self.status_distribution[status] += 1
        if status == "MERGED":
            created_at = datetime.fromisoformat(pr.get("createdAt"))
            merged_at = datetime.fromisoformat(pr.get("mergedAt"))
            self.merge_time.append((merged_at - created_at).total_seconds() / 3600)

        # Process reviews
        reviews = pr.get("reviews", {}).get("nodes", [])
        reviews = list(
            filter(lambda r: self.get_login(r.get("author")) != pr_author, reviews)
        )
        if len(reviews) == 0 and pr_author == merged_by:
            self.self_approved_prs += 1
        else:
            self.approved_prs += 1
