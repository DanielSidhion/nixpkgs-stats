from files import process_files, write_to_js_file
from collections import Counter


def prepare_variables(data):
    # most frequent commit dates
    common_dates = [
        date.strftime("%Y-%m-%d")
        for date, _ in Counter(data.commit_dates).most_common(5)
    ]
    common_dates_counts = [
        count for _, count in Counter(data.commit_dates).most_common(5)
    ]

    # day-wise commit counts
    daywise_commit_counts = Counter(
        commit_date.weekday() for commit_date in data.commit_dates
    )
    days = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ]
    data_days = [daywise_commit_counts[i] for i in range(7)]

    # month-wise commit counts
    monthwise_commit_counts = Counter(
        commit_date.month for commit_date in data.commit_dates
    )
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
    labels_pr_authors = list(data.all_prs_per_author.keys())
    data_pr_authors = list(data.all_prs_per_author.values())

    # status distribution of PRs data
    labels_statuses = list(data.status_distribution.keys())
    data_statuses = list(data.status_distribution.values())

    # avg time taken to merge PRs
    average_merge_time = (
        sum(data.merge_time) / len(data.merge_time) if data.merge_time else 0
    )

    labels_pr_approval = ["Approved by Others", "Self-Approved"]
    data_pr_approval = [data.approved_prs, data.self_approved_prs]

    yearwise_commit_counts = Counter(
        commit_date.year for commit_date in data.commit_dates
    )
    years = sorted(yearwise_commit_counts.keys())
    data_years = [yearwise_commit_counts[year] for year in years]

    variables = {
        "labels_authors": list(data.all_authors.keys()),
        "data_authors": list(data.all_authors.values()),
        "common_dates": common_dates,
        "common_dates_counts": common_dates_counts,
        "days": days,
        "data_days": data_days,
        "months": months,
        "data_months": data_months,
        "labels_pr_authors": labels_pr_authors,
        "data_pr_authors": data_pr_authors,
        "labels_statuses": labels_statuses,
        "data_statuses": data_statuses,
        "average_merge_time": average_merge_time,
        "labels_pr_approval": labels_pr_approval,
        "data_pr_approval": data_pr_approval,
        "years": years,
        "data_years": data_years,
    }

    return variables


def main():
    data = process_files()
    variables = prepare_variables(data)
    write_to_js_file(variables)


if __name__ == "__main__":
    main()
