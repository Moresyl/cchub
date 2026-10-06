import { useId, useState } from "react";
import { BarChart3, ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "../../components/ui/button";

const PAGE_SIZE = 12;

export function AggregateTable({
  title,
  headers,
  rows,
  empty,
  uiText,
}: {
  title: string;
  headers: string[];
  rows: string[][];
  empty: string;
  uiText: (zh: string, en: string, ja: string) => string;
}) {
  const titleId = useId();
  const [page, setPage] = useState(0);
  const lastPage = Math.max(0, Math.ceil(rows.length / PAGE_SIZE) - 1);
  const currentPage = Math.min(page, lastPage);
  const start = currentPage * PAGE_SIZE;
  return (
    <section className="section-card usage-ranking" aria-labelledby={titleId}>
      <h2 className="section-card-title" id={titleId}>
        <BarChart3 size={16} aria-hidden="true" />
        {title}
      </h2>
      {!rows.length ? (
        <p className="state-copy">{empty}</p>
      ) : (
        <>
          <ul className="usage-compact-rows" aria-label={uiText(`${title}列表`, `${title} list`, `${title}の一覧`)}>
            {rows.slice(start, start + PAGE_SIZE).map((row, index) => (
              <li key={`${row[0]}-${index}`}>
                <h3>{row[0]}</h3>
                <dl>
                  {row.slice(1).map((cell, cellIndex) => (
                    <div key={headers[cellIndex + 1]}>
                      <dt>{headers[cellIndex + 1]}</dt>
                      <dd>{cell}</dd>
                    </div>
                  ))}
                </dl>
              </li>
            ))}
          </ul>
          <div className="usage-table-scroll" role="region" aria-labelledby={titleId} tabIndex={0}>
            <table className="data-table usage-table" aria-labelledby={titleId}>
              <thead>
                <tr>
                  {headers.map((header) => (
                    <th key={header} scope="col">
                      {header}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.slice(start, start + PAGE_SIZE).map((row, index) => (
                  <tr key={`${row[0]}-${index}`}>
                    {row.map((cell, cellIndex) => (
                      <td key={cellIndex} title={cell}>
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {rows.length > PAGE_SIZE ? (
            <nav
              className="usage-pagination"
              aria-label={uiText(`${title}分页`, `${title} pagination`, `${title}のページ切替`)}
            >
              <span role="status">
                {start + 1}–{Math.min(start + PAGE_SIZE, rows.length)} / {rows.length}
              </span>
              <div className="flex gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={uiText("上一页", "Previous page", "前のページ")}
                  disabled={currentPage === 0}
                  onClick={() => setPage(currentPage - 1)}
                >
                  <ChevronLeft size={16} />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={uiText("下一页", "Next page", "次のページ")}
                  disabled={currentPage === lastPage}
                  onClick={() => setPage(currentPage + 1)}
                >
                  <ChevronRight size={16} />
                </Button>
              </div>
            </nav>
          ) : null}
        </>
      )}
    </section>
  );
}
