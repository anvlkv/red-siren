#!/usr/bin/env python3
"""
NIST WebBook â€” Fluid System: extract Earth-normal fluid data for every
substance in the dropdown and write a single CSV with a Substance column.

Workflow per substance, executed via Playwright through the real UI:

    1. Open https://webbook.nist.gov/chemistry/fluid/
    2. Pick the species in <select name="ID"> dropdown.
    3. Choose units: Temperature = C, Density = kg/m3, Viscosity = Pa*s
       (other units left at defaults: P = MPa, energies = kJ/kg, etc.).
    4. Choose data Type = "Isobaric properties".
    5. Submit "Press to Continue".
    6. Fill P=0.101325 MPa, TLow=0 C, THigh=100 C, TInc=5 C
       (Earth-normal: 1 atm, 0..100 C, every 5 C).
    7. Submit "Press for Data".
    8. Follow the "Download data" link (tab-delimited text file).
    9. Append rows to ONE combined CSV with a Substance column.

Usage:
    pip install playwright
    playwright install chromium
    python nist_fluid_playwright.py
        [--out nist_fluid_data.csv]
        [--headless 1|0]
        [--limit N]      # debug: process only first N substances
"""

import argparse
import csv
from pathlib import Path

from playwright.sync_api import sync_playwright

LANDING_URL = "https://webbook.nist.gov/chemistry/fluid/"

P_MPA   = "0.101325"   # 1 atm
T_LOW   = "0"
T_HIGH  = "100"
T_INC   = "5"


def harvest_substances(page):
    page.goto(LANDING_URL, wait_until="domcontentloaded")
    return page.eval_on_selector_all(
        'select[name="ID"] option',
        "els => els.map(e => [e.value, e.textContent.trim()])",
    )


def fetch_one(page, species_id, species_name):
    """Drive the UI for one substance and return (header, rows) parsed
    from the tab-delimited download. header is None on failure."""

    page.goto(LANDING_URL, wait_until="domcontentloaded")

    page.select_option('select[name="ID"]', species_id)
    page.check('input[name="TUnit"][value="C"]')
    page.check('input[name="DUnit"][value="kg/m3"]')
    page.check('input[name="VisUnit"][value="Pa*s"]')
    page.check('input[name="Type"][value="IsoBar"]')

    with page.expect_navigation(wait_until="domcontentloaded"):
        page.click('input[type="submit"][value="Press to Continue"]')

    page.fill('input[name="P"]',     P_MPA)
    page.fill('input[name="TLow"]',  T_LOW)
    page.fill('input[name="THigh"]', T_HIGH)
    page.fill('input[name="TInc"]',  T_INC)

    with page.expect_navigation(wait_until="domcontentloaded"):
        page.click('input[type="submit"][value="Press for Data"]')

    href = page.get_attribute('a:has-text("Download data")', "href")
    if not href:
        return None, []
    if href.startswith("/"):
        href = "https://webbook.nist.gov" + href

    resp = page.request.get(href)
    if not resp.ok:
        return None, []

    lines = [ln for ln in resp.text().splitlines() if ln.strip()]
    if not lines:
        return None, []
    header = lines[0].split("\t")
    rows   = [ln.split("\t") for ln in lines[1:]]
    return header, rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="nist_fluid_data.csv")
    ap.add_argument("--headless", default="1")
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()

    out_path = Path(args.out)
    headless = args.headless not in ("0", "false", "False")

    with sync_playwright() as pw:
        browser = pw.chromium.launch(headless=headless)
        page    = browser.new_context().new_page()

        substances = harvest_substances(page)
        if args.limit:
            substances = substances[: args.limit]
        print(f"Found {len(substances)} substances")

        writer  = None
        fh      = out_path.open("w", newline="")
        n_rows  = 0
        try:
            for i, (sid, sname) in enumerate(substances, 1):
                try:
                    header, rows = fetch_one(page, sid, sname)
                except Exception as exc:
                    print(f"[{i:>3}/{len(substances)}] {sname}: ERROR {exc}")
                    continue
                if header is None:
                    print(f"[{i:>3}/{len(substances)}] {sname}: no data")
                    continue

                if writer is None:
                    writer = csv.writer(fh)
                    writer.writerow(["Substance", "ID"] + header)

                for row in rows:
                    writer.writerow([sname, sid] + row)
                n_rows += len(rows)
                fh.flush()
                print(f"[{i:>3}/{len(substances)}] {sname}: +{len(rows)} rows "
                      f"(total {n_rows})")
        finally:
            fh.close()
            browser.close()

        print(f"Wrote {n_rows} data rows to {out_path}")


if __name__ == "__main__":
    main()