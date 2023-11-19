import glob
import os
import json
from collections import Counter
from datetime import datetime

data_directory = "data"

all_authors = {}
commit_dates = []

all_prs_per_author = {}
status_distribution = Counter()
merge_time = []
review_comments_count = []

# start with zeroes, increment manually as the data is read
approved_prs = 0  # PRs with approvals from others
self_approved_prs = 0  # Self-approved PRs

def get_login(value):
    if value is None:
        return None
    return value.get("login")

for file_path in glob.glob(os.path.join(data_directory, "raw_*.json")):
    json_data = None

    with open(file_path, "r") as file:
        try:
            json_data = json.load(file)
        except (json.JSONDecodeError, KeyError) as e:
            print(f"Error parsing {file_path}: {e}")
            continue
    
    if json_data is None:
        print(f"Parsed {file_path}, but got nothing.")
        continue
        
    for commit in json_data:
        authors = commit.get("authors", {}).get("nodes", [])
        for author in authors:
            author_name = author.get("name")
            if author_name:
                all_authors[author_name] = all_authors.get(author_name, 0) + 1

        commit_date = commit.get("committedDate")
        if commit_date:
            commit_dates.append(datetime.fromisoformat(commit_date).date())

        prs = commit.get("associatedPullRequests", {}).get("nodes", [])
        prs = filter(lambda pr: isinstance(pr, dict), prs)
        for pr in prs:
            pr_author = get_login(pr.get("author"))
            merged_by = get_login(pr.get("mergedBy"))
            status = pr.get("state")
            if not pr_author or not merged_by or not status:
                # There are a bunch of cases like this, so we'll just skip instead of printing each case. Must fix the raw data later.
                continue

            all_prs_per_author[pr_author] = all_prs_per_author.get(pr_author, 0) + 1

            status_distribution[status] += 1
            if status == "MERGED":
                created_at = datetime.fromisoformat(pr.get("createdAt"))
                merged_at = datetime.fromisoformat(pr.get("mergedAt"))
                merge_time.append((merged_at - created_at).total_seconds() / 3600)
            
            # Only consider self-approved prs if the author and merger are the same, and there are no other reviews after excluding reviews by the pr author.
            reviews = pr.get("reviews", {}).get("nodes", [])
            reviews = list(filter(lambda r: get_login(r.get("author")) != pr_author, reviews))
            if len(reviews) == 0 and pr_author == merged_by:
                self_approved_prs += 1
            else:
                approved_prs += 1

# prep. data for Chart.js
labels_authors = list(all_authors.keys())
data_authors = list(all_authors.values())

# most frequent commit dates
common_dates = [
    date.strftime("%Y-%m-%d") for date, _ in Counter(commit_dates).most_common(5)
]
common_dates_counts = [count for _, count in Counter(commit_dates).most_common(5)]

# day-wise commit counts
daywise_commit_counts = Counter(commit_date.weekday() for commit_date in commit_dates)
days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
data_days = [daywise_commit_counts[i] for i in range(7)]

# month-wise commit counts
monthwise_commit_counts = Counter(commit_date.month for commit_date in commit_dates)
months = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]

data_months = [monthwise_commit_counts[i] for i in range(1, 13)]

# PRs per author data
labels_pr_authors = list(all_prs_per_author.keys())
data_pr_authors = list(all_prs_per_author.values())

# status distribution of PRs data
labels_statuses = list(status_distribution.keys())
data_statuses = list(status_distribution.values())

# avg time taken to merge PRs
average_merge_time = sum(merge_time) / len(merge_time) if merge_time else 0

# avg review comments per PR
average_review_comments = (
    sum(review_comments_count) / len(review_comments_count)
    if review_comments_count
    else 0
)

labels_pr_approval = ["Approved by Others", "Self-Approved"]
data_pr_approval = [approved_prs, self_approved_prs]

yearwise_commit_counts = Counter(commit_date.year for commit_date in commit_dates)
years = sorted(yearwise_commit_counts.keys())
data_years = [yearwise_commit_counts[year] for year in years]


# construct chartjs data
with open("chart_data.js", "w") as js_file:
    js_file.write(f"const labels_authors = {json.dumps(labels_authors)};\n")
    js_file.write(f"const data_authors = {json.dumps(data_authors)};\n")
    js_file.write(f"const common_dates = {json.dumps(common_dates)};\n")
    js_file.write(f"const common_dates_counts = {json.dumps(common_dates_counts)};\n")
    js_file.write(f"const days = {json.dumps(days)};\n")
    js_file.write(f"const data_days = {json.dumps(data_days)};\n")
    js_file.write(f"const months = {json.dumps(months)};\n")
    js_file.write(f"const data_months = {json.dumps(data_months)};\n")
    js_file.write(f"const labels_pr_authors = {json.dumps(labels_pr_authors)};\n")
    js_file.write(f"const data_pr_authors = {json.dumps(data_pr_authors)};\n")
    js_file.write(f"const labels_statuses = {json.dumps(labels_statuses)};\n")
    js_file.write(f"const data_statuses = {json.dumps(data_statuses)};\n")
    js_file.write(f"const average_merge_time = {average_merge_time};\n")
    js_file.write(f"const average_review_comments = {average_review_comments};\n")
    js_file.write(f"const labels_pr_approval = {json.dumps(labels_pr_approval)};\n")
    js_file.write(f"const data_pr_approval = {json.dumps(data_pr_approval)};\n")
    js_file.write(f"const years = {json.dumps(years)};\n")
    js_file.write(f"const data_years = {json.dumps(data_years)};\n")
