# NS Accounts — Accounting Core Audit

**Audit baseline:** `feature/accounting-v1.2-reports-company`  
**Status:** Static source review started; no claim of runtime or Windows verification.

## Release decision

**Not suitable for live books or statutory accounting yet.** The current build is a prototype. The UI launching and TypeScript/Vite build succeeding do not establish accounting correctness.

## Critical findings from the source review

| Area | Finding | Risk | Required correction |
|---|---|---:|---|
| Company onboarding | A single hard-coded company row is inserted automatically. | Critical | Explicit company creation and isolated books per company. Demo data must be opt-in and clearly labelled. |
| Opening balances | Each ledger accepts an independent opening balance. No balanced opening journal/control account is enforced. | Critical | Opening-balance import/workflow must validate total Dr = total Cr before activation. |
| Financial year | Trial Balance aggregates every voucher without a date boundary. | Critical | All reports must use explicit as-of/period dates and enforce company FY rules. |
| Voucher rules | The backend accepts a free-form voucher type and does not enforce Contra/Payment/Receipt/Journal account-side rules. | High | Validate allowed type and its accounting constraints in the backend. |
| Duplicate posting | Frontend has an in-memory posting guard, but there is no idempotency token or database-level request identity. | High | Add durable idempotency protection and regression tests for rapid/repeated submission. |
| Blank voucher rows | Frontend filters lines with zero debit and credit. This must be tested alongside row focus/add/remove and repeated keyboard submit. | High | Validate only populated rows, reject partially populated rows, and prove one post per user action. |
| Date validation | Backend currently checks only that date text has length 10. | High | Parse strict ISO dates and validate period boundaries. |
| Reports | P&L and Balance Sheet are group-name aggregation previews; closing profit, group hierarchy and full presentation are absent. | Critical | Build report logic from a formal chart-of-accounts hierarchy and independently reconcile totals. |
| Voucher lifecycle | Posted vouchers are read-only; no alteration, cancellation, reversal or audit trail exists. | High | Implement controlled alteration/cancellation with immutable audit history. |
| Database evolution | Tables are created with `IF NOT EXISTS`; no schema version/migration framework is present. | High | Introduce versioned, transactional migrations and backup-before-upgrade. |
| Data safety | No backup/restore workflow or tested recovery path is present. | Critical | Implement verified backup, restore, integrity check and recovery tests before production use. |
| Keyboard UX | The UI uses a global key listener and local form key handling. Shortcut behaviour varies with focused controls and needs a central, tested key map. | Medium | Centralize shortcut routing and test every workflow without mouse input. |

## Required accounting acceptance tests

The test suite must seed a disposable company and verify both ledger-level movements and report totals.

1. Capital introduced: Cash Dr / Capital Cr.
2. Contra: Bank Dr / Cash Cr.
3. Credit purchase: Purchase Dr / Supplier Cr.
4. Cash sale: Cash Dr / Sales Cr.
5. Purchase return: Supplier Dr / Purchase Return Cr.
6. Supplier payment: Supplier Dr / Bank Cr.
7. Multi-line voucher: multiple debit and credit lines balance exactly.
8. Blank trailing row: ignored safely; it must not create an empty line or duplicate voucher.
9. Partially completed row: rejected with a precise validation message.
10. Repeated Ctrl+Enter / rapid double-click: one voucher only.
11. Unbalanced voucher: rejected and database remains unchanged.
12. Unknown ledger: rejected and database remains unchanged.
13. Invalid calendar date and out-of-period date: rejected according to company policy.
14. Trial Balance: independently calculated Dr and Cr totals agree.
15. Ledger statement: opening balance, period movement and closing balance reconcile.
16. P&L and Balance Sheet: reconcile to the same trial balance and period.
17. Cancellation/reversal: original posting remains auditable and balances remain correct.
18. Backup/restore: restored database matches source balances and voucher counts.

## Engineering sequence

1. Freeze feature expansion and preserve existing user data.
2. Add pure accounting-domain functions and automated unit tests.
3. Add versioned database migrations and company isolation.
4. Replace independent opening balances with a balanced opening journal workflow.
5. Harden voucher validation and idempotent posting.
6. Rebuild date-aware accounting reports and reconcile them against test fixtures.
7. Centralize keyboard navigation and add keyboard-only UI tests.
8. Add voucher alteration/cancellation audit history.
9. Add backup/restore and recovery tests.
10. Only then resume GST and inventory feature development.

## Verification policy

- Every accounting calculation needs an automated expected-result test.
- Every database write needs success, rejection and rollback tests.
- Every release candidate needs a clean frontend build, Rust test/build, Windows desktop launch test and a manual accounting acceptance run.
- A feature is not marked complete because its screen renders.
- Existing local databases must be backed up before testing builds that change the schema.
