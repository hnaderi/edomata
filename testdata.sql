--
-- PostgreSQL database dump
--

-- Dumped from database version 13.4 (Debian 13.4-1.pgdg100+1)
-- Dumped by pg_dump version 14.1

-- Started on 2022-05-02 17:53:04 +0430

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- TOC entry 7 (class 2615 OID 22085)
-- Name: compatibility_binary; Type: SCHEMA; Schema: -; Owner: postgres
--

CREATE SCHEMA compatibility_binary;


ALTER SCHEMA compatibility_binary OWNER TO postgres;

--
-- TOC entry 9 (class 2615 OID 22015)
-- Name: compatibility_json; Type: SCHEMA; Schema: -; Owner: postgres
--

CREATE SCHEMA compatibility_json;


ALTER SCHEMA compatibility_json OWNER TO postgres;

--
-- TOC entry 3 (class 2615 OID 22050)
-- Name: compatibility_jsonb; Type: SCHEMA; Schema: -; Owner: postgres
--

CREATE SCHEMA compatibility_jsonb;


ALTER SCHEMA compatibility_jsonb OWNER TO postgres;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- TOC entry 250 (class 1259 OID 22112)
-- Name: commands; Type: TABLE; Schema: compatibility_binary; Owner: postgres
--

CREATE TABLE compatibility_binary.commands (
    id text NOT NULL,
    "time" timestamp with time zone NOT NULL,
    address text NOT NULL
);


ALTER TABLE compatibility_binary.commands OWNER TO postgres;

--
-- TOC entry 247 (class 1259 OID 22088)
-- Name: journal; Type: TABLE; Schema: compatibility_binary; Owner: postgres
--

CREATE TABLE compatibility_binary.journal (
    id uuid NOT NULL,
    "time" timestamp with time zone NOT NULL,
    seqnr bigint NOT NULL,
    version bigint NOT NULL,
    stream text NOT NULL,
    payload bytea NOT NULL
);


ALTER TABLE compatibility_binary.journal OWNER TO postgres;

--
-- TOC entry 246 (class 1259 OID 22086)
-- Name: journal_seqnr_seq; Type: SEQUENCE; Schema: compatibility_binary; Owner: postgres
--

CREATE SEQUENCE compatibility_binary.journal_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_binary.journal_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3175 (class 0 OID 0)
-- Dependencies: 246
-- Name: journal_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_binary; Owner: postgres
--

ALTER SEQUENCE compatibility_binary.journal_seqnr_seq OWNED BY compatibility_binary.journal.seqnr;


--
-- TOC entry 249 (class 1259 OID 22103)
-- Name: outbox; Type: TABLE; Schema: compatibility_binary; Owner: postgres
--

CREATE TABLE compatibility_binary.outbox (
    seqnr bigint NOT NULL,
    stream text NOT NULL,
    correlation text,
    causation text,
    payload bytea NOT NULL,
    created timestamp with time zone NOT NULL,
    published timestamp with time zone
);


ALTER TABLE compatibility_binary.outbox OWNER TO postgres;

--
-- TOC entry 248 (class 1259 OID 22101)
-- Name: outbox_seqnr_seq; Type: SEQUENCE; Schema: compatibility_binary; Owner: postgres
--

CREATE SEQUENCE compatibility_binary.outbox_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_binary.outbox_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3176 (class 0 OID 0)
-- Dependencies: 248
-- Name: outbox_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_binary; Owner: postgres
--

ALTER SEQUENCE compatibility_binary.outbox_seqnr_seq OWNED BY compatibility_binary.outbox.seqnr;


--
-- TOC entry 251 (class 1259 OID 22120)
-- Name: snapshots; Type: TABLE; Schema: compatibility_binary; Owner: postgres
--

CREATE TABLE compatibility_binary.snapshots (
    id text NOT NULL,
    version bigint NOT NULL,
    state bytea NOT NULL
);


ALTER TABLE compatibility_binary.snapshots OWNER TO postgres;

--
-- TOC entry 240 (class 1259 OID 22042)
-- Name: commands; Type: TABLE; Schema: compatibility_json; Owner: postgres
--

CREATE TABLE compatibility_json.commands (
    id text NOT NULL,
    "time" timestamp with time zone NOT NULL,
    address text NOT NULL
);


ALTER TABLE compatibility_json.commands OWNER TO postgres;

--
-- TOC entry 237 (class 1259 OID 22018)
-- Name: journal; Type: TABLE; Schema: compatibility_json; Owner: postgres
--

CREATE TABLE compatibility_json.journal (
    id uuid NOT NULL,
    "time" timestamp with time zone NOT NULL,
    seqnr bigint NOT NULL,
    version bigint NOT NULL,
    stream text NOT NULL,
    payload json NOT NULL
);


ALTER TABLE compatibility_json.journal OWNER TO postgres;

--
-- TOC entry 236 (class 1259 OID 22016)
-- Name: journal_seqnr_seq; Type: SEQUENCE; Schema: compatibility_json; Owner: postgres
--

CREATE SEQUENCE compatibility_json.journal_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_json.journal_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3177 (class 0 OID 0)
-- Dependencies: 236
-- Name: journal_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_json; Owner: postgres
--

ALTER SEQUENCE compatibility_json.journal_seqnr_seq OWNED BY compatibility_json.journal.seqnr;


--
-- TOC entry 239 (class 1259 OID 22033)
-- Name: outbox; Type: TABLE; Schema: compatibility_json; Owner: postgres
--

CREATE TABLE compatibility_json.outbox (
    seqnr bigint NOT NULL,
    stream text NOT NULL,
    correlation text,
    causation text,
    payload json NOT NULL,
    created timestamp with time zone NOT NULL,
    published timestamp with time zone
);


ALTER TABLE compatibility_json.outbox OWNER TO postgres;

--
-- TOC entry 238 (class 1259 OID 22031)
-- Name: outbox_seqnr_seq; Type: SEQUENCE; Schema: compatibility_json; Owner: postgres
--

CREATE SEQUENCE compatibility_json.outbox_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_json.outbox_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3178 (class 0 OID 0)
-- Dependencies: 238
-- Name: outbox_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_json; Owner: postgres
--

ALTER SEQUENCE compatibility_json.outbox_seqnr_seq OWNED BY compatibility_json.outbox.seqnr;


--
-- TOC entry 253 (class 1259 OID 22136)
-- Name: snapshots; Type: TABLE; Schema: compatibility_json; Owner: postgres
--

CREATE TABLE compatibility_json.snapshots (
    id text NOT NULL,
    version bigint NOT NULL,
    state json NOT NULL
);


ALTER TABLE compatibility_json.snapshots OWNER TO postgres;

--
-- TOC entry 245 (class 1259 OID 22077)
-- Name: commands; Type: TABLE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE TABLE compatibility_jsonb.commands (
    id text NOT NULL,
    "time" timestamp with time zone NOT NULL,
    address text NOT NULL
);


ALTER TABLE compatibility_jsonb.commands OWNER TO postgres;

--
-- TOC entry 242 (class 1259 OID 22053)
-- Name: journal; Type: TABLE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE TABLE compatibility_jsonb.journal (
    id uuid NOT NULL,
    "time" timestamp with time zone NOT NULL,
    seqnr bigint NOT NULL,
    version bigint NOT NULL,
    stream text NOT NULL,
    payload jsonb NOT NULL
);


ALTER TABLE compatibility_jsonb.journal OWNER TO postgres;

--
-- TOC entry 241 (class 1259 OID 22051)
-- Name: journal_seqnr_seq; Type: SEQUENCE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE SEQUENCE compatibility_jsonb.journal_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_jsonb.journal_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3179 (class 0 OID 0)
-- Dependencies: 241
-- Name: journal_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_jsonb; Owner: postgres
--

ALTER SEQUENCE compatibility_jsonb.journal_seqnr_seq OWNED BY compatibility_jsonb.journal.seqnr;


--
-- TOC entry 244 (class 1259 OID 22068)
-- Name: outbox; Type: TABLE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE TABLE compatibility_jsonb.outbox (
    seqnr bigint NOT NULL,
    stream text NOT NULL,
    correlation text,
    causation text,
    payload jsonb NOT NULL,
    created timestamp with time zone NOT NULL,
    published timestamp with time zone
);


ALTER TABLE compatibility_jsonb.outbox OWNER TO postgres;

--
-- TOC entry 243 (class 1259 OID 22066)
-- Name: outbox_seqnr_seq; Type: SEQUENCE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE SEQUENCE compatibility_jsonb.outbox_seqnr_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE compatibility_jsonb.outbox_seqnr_seq OWNER TO postgres;

--
-- TOC entry 3180 (class 0 OID 0)
-- Dependencies: 243
-- Name: outbox_seqnr_seq; Type: SEQUENCE OWNED BY; Schema: compatibility_jsonb; Owner: postgres
--

ALTER SEQUENCE compatibility_jsonb.outbox_seqnr_seq OWNED BY compatibility_jsonb.outbox.seqnr;


--
-- TOC entry 252 (class 1259 OID 22128)
-- Name: snapshots; Type: TABLE; Schema: compatibility_jsonb; Owner: postgres
--

CREATE TABLE compatibility_jsonb.snapshots (
    id text NOT NULL,
    version bigint NOT NULL,
    state jsonb NOT NULL
);


ALTER TABLE compatibility_jsonb.snapshots OWNER TO postgres;

--
-- TOC entry 2984 (class 2604 OID 22091)
-- Name: journal seqnr; Type: DEFAULT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.journal ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_binary.journal_seqnr_seq'::regclass);


--
-- TOC entry 2985 (class 2604 OID 22106)
-- Name: outbox seqnr; Type: DEFAULT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.outbox ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_binary.outbox_seqnr_seq'::regclass);


--
-- TOC entry 2980 (class 2604 OID 22021)
-- Name: journal seqnr; Type: DEFAULT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.journal ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_json.journal_seqnr_seq'::regclass);


--
-- TOC entry 2981 (class 2604 OID 22036)
-- Name: outbox seqnr; Type: DEFAULT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.outbox ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_json.outbox_seqnr_seq'::regclass);


--
-- TOC entry 2982 (class 2604 OID 22056)
-- Name: journal seqnr; Type: DEFAULT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.journal ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_jsonb.journal_seqnr_seq'::regclass);


--
-- TOC entry 2983 (class 2604 OID 22071)
-- Name: outbox seqnr; Type: DEFAULT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.outbox ALTER COLUMN seqnr SET DEFAULT nextval('compatibility_jsonb.outbox_seqnr_seq'::regclass);


--
-- TOC entry 3166 (class 0 OID 22112)
-- Dependencies: 250
-- Data for Name: commands; Type: TABLE DATA; Schema: compatibility_binary; Owner: postgres
--

COPY compatibility_binary.commands (id, "time", address) FROM stdin;
redundant	1970-01-01 00:00:00+00	a
\.


--
-- TOC entry 3163 (class 0 OID 22088)
-- Dependencies: 247
-- Data for Name: journal; Type: TABLE DATA; Schema: compatibility_binary; Owner: postgres
--

COPY compatibility_binary.journal (id, "time", seqnr, version, stream, payload) FROM stdin;
00000000-0000-0000-0000-000000000000	1970-01-01 00:00:00+00	1	0	a	\\x1234
\.


--
-- TOC entry 3165 (class 0 OID 22103)
-- Dependencies: 249
-- Data for Name: outbox; Type: TABLE DATA; Schema: compatibility_binary; Owner: postgres
--

COPY compatibility_binary.outbox (seqnr, stream, correlation, causation, payload, created, published) FROM stdin;
1	a	correlation	causation	\\x123456	1970-01-01 00:00:00+00	\N
\.


--
-- TOC entry 3167 (class 0 OID 22120)
-- Dependencies: 251
-- Data for Name: snapshots; Type: TABLE DATA; Schema: compatibility_binary; Owner: postgres
--

COPY compatibility_binary.snapshots (id, version, state) FROM stdin;
a	1	\\x12
\.


--
-- TOC entry 3156 (class 0 OID 22042)
-- Dependencies: 240
-- Data for Name: commands; Type: TABLE DATA; Schema: compatibility_json; Owner: postgres
--

COPY compatibility_json.commands (id, "time", address) FROM stdin;
abc	2022-05-02 12:55:52.736254+00	a
redundant	1970-01-01 00:00:00+00	a
\.


--
-- TOC entry 3153 (class 0 OID 22018)
-- Dependencies: 237
-- Data for Name: journal; Type: TABLE DATA; Schema: compatibility_json; Owner: postgres
--

COPY compatibility_json.journal (id, "time", seqnr, version, stream, payload) FROM stdin;
00000000-0000-0000-0000-000000000000	1970-01-01 00:00:00+00	1	0	a	1234
\.


--
-- TOC entry 3155 (class 0 OID 22033)
-- Dependencies: 239
-- Data for Name: outbox; Type: TABLE DATA; Schema: compatibility_json; Owner: postgres
--

COPY compatibility_json.outbox (seqnr, stream, correlation, causation, payload, created, published) FROM stdin;
1	a	correlation	causation	123456	1970-01-01 00:00:00+00	\N
\.


--
-- TOC entry 3169 (class 0 OID 22136)
-- Dependencies: 253
-- Data for Name: snapshots; Type: TABLE DATA; Schema: compatibility_json; Owner: postgres
--

COPY compatibility_json.snapshots (id, version, state) FROM stdin;
a	1	12
\.


--
-- TOC entry 3161 (class 0 OID 22077)
-- Dependencies: 245
-- Data for Name: commands; Type: TABLE DATA; Schema: compatibility_jsonb; Owner: postgres
--

COPY compatibility_jsonb.commands (id, "time", address) FROM stdin;
redundant	1970-01-01 00:00:00+00	a
\.


--
-- TOC entry 3158 (class 0 OID 22053)
-- Dependencies: 242
-- Data for Name: journal; Type: TABLE DATA; Schema: compatibility_jsonb; Owner: postgres
--

COPY compatibility_jsonb.journal (id, "time", seqnr, version, stream, payload) FROM stdin;
00000000-0000-0000-0000-000000000000	1970-01-01 00:00:00+00	1	0	a	1234
\.


--
-- TOC entry 3160 (class 0 OID 22068)
-- Dependencies: 244
-- Data for Name: outbox; Type: TABLE DATA; Schema: compatibility_jsonb; Owner: postgres
--

COPY compatibility_jsonb.outbox (seqnr, stream, correlation, causation, payload, created, published) FROM stdin;
1	a	correlation	causation	123456	1970-01-01 00:00:00+00	\N
\.


--
-- TOC entry 3168 (class 0 OID 22128)
-- Dependencies: 252
-- Data for Name: snapshots; Type: TABLE DATA; Schema: compatibility_jsonb; Owner: postgres
--

COPY compatibility_jsonb.snapshots (id, version, state) FROM stdin;
a	1	12
\.


--
-- TOC entry 3181 (class 0 OID 0)
-- Dependencies: 246
-- Name: journal_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_binary; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_binary.journal_seqnr_seq', 2, true);


--
-- TOC entry 3182 (class 0 OID 0)
-- Dependencies: 248
-- Name: outbox_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_binary; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_binary.outbox_seqnr_seq', 1, true);


--
-- TOC entry 3183 (class 0 OID 0)
-- Dependencies: 236
-- Name: journal_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_json; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_json.journal_seqnr_seq', 2, true);


--
-- TOC entry 3184 (class 0 OID 0)
-- Dependencies: 238
-- Name: outbox_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_json; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_json.outbox_seqnr_seq', 1, true);


--
-- TOC entry 3185 (class 0 OID 0)
-- Dependencies: 241
-- Name: journal_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_jsonb; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_jsonb.journal_seqnr_seq', 2, true);


--
-- TOC entry 3186 (class 0 OID 0)
-- Dependencies: 243
-- Name: outbox_seqnr_seq; Type: SEQUENCE SET; Schema: compatibility_jsonb; Owner: postgres
--

SELECT pg_catalog.setval('compatibility_jsonb.outbox_seqnr_seq', 1, true);


--
-- TOC entry 3015 (class 2606 OID 22119)
-- Name: commands commands_pk; Type: CONSTRAINT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.commands
    ADD CONSTRAINT commands_pk PRIMARY KEY (id);


--
-- TOC entry 3007 (class 2606 OID 22096)
-- Name: journal journal_pk; Type: CONSTRAINT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.journal
    ADD CONSTRAINT journal_pk PRIMARY KEY (id);


--
-- TOC entry 3011 (class 2606 OID 22098)
-- Name: journal journal_un; Type: CONSTRAINT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.journal
    ADD CONSTRAINT journal_un UNIQUE (stream, version);


--
-- TOC entry 3013 (class 2606 OID 22111)
-- Name: outbox outbox_pk; Type: CONSTRAINT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.outbox
    ADD CONSTRAINT outbox_pk PRIMARY KEY (seqnr);


--
-- TOC entry 3017 (class 2606 OID 22127)
-- Name: snapshots snapshots_pk; Type: CONSTRAINT; Schema: compatibility_binary; Owner: postgres
--

ALTER TABLE ONLY compatibility_binary.snapshots
    ADD CONSTRAINT snapshots_pk PRIMARY KEY (id);


--
-- TOC entry 2995 (class 2606 OID 22049)
-- Name: commands commands_pk; Type: CONSTRAINT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.commands
    ADD CONSTRAINT commands_pk PRIMARY KEY (id);


--
-- TOC entry 2987 (class 2606 OID 22026)
-- Name: journal journal_pk; Type: CONSTRAINT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.journal
    ADD CONSTRAINT journal_pk PRIMARY KEY (id);


--
-- TOC entry 2991 (class 2606 OID 22028)
-- Name: journal journal_un; Type: CONSTRAINT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.journal
    ADD CONSTRAINT journal_un UNIQUE (stream, version);


--
-- TOC entry 2993 (class 2606 OID 22041)
-- Name: outbox outbox_pk; Type: CONSTRAINT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.outbox
    ADD CONSTRAINT outbox_pk PRIMARY KEY (seqnr);


--
-- TOC entry 3021 (class 2606 OID 22143)
-- Name: snapshots snapshots_pk; Type: CONSTRAINT; Schema: compatibility_json; Owner: postgres
--

ALTER TABLE ONLY compatibility_json.snapshots
    ADD CONSTRAINT snapshots_pk PRIMARY KEY (id);


--
-- TOC entry 3005 (class 2606 OID 22084)
-- Name: commands commands_pk; Type: CONSTRAINT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.commands
    ADD CONSTRAINT commands_pk PRIMARY KEY (id);


--
-- TOC entry 2997 (class 2606 OID 22061)
-- Name: journal journal_pk; Type: CONSTRAINT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.journal
    ADD CONSTRAINT journal_pk PRIMARY KEY (id);


--
-- TOC entry 3001 (class 2606 OID 22063)
-- Name: journal journal_un; Type: CONSTRAINT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.journal
    ADD CONSTRAINT journal_un UNIQUE (stream, version);


--
-- TOC entry 3003 (class 2606 OID 22076)
-- Name: outbox outbox_pk; Type: CONSTRAINT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.outbox
    ADD CONSTRAINT outbox_pk PRIMARY KEY (seqnr);


--
-- TOC entry 3019 (class 2606 OID 22135)
-- Name: snapshots snapshots_pk; Type: CONSTRAINT; Schema: compatibility_jsonb; Owner: postgres
--

ALTER TABLE ONLY compatibility_jsonb.snapshots
    ADD CONSTRAINT snapshots_pk PRIMARY KEY (id);


--
-- TOC entry 3008 (class 1259 OID 22099)
-- Name: journal_seqnr_idx; Type: INDEX; Schema: compatibility_binary; Owner: postgres
--

CREATE INDEX journal_seqnr_idx ON compatibility_binary.journal USING btree (seqnr);


--
-- TOC entry 3009 (class 1259 OID 22100)
-- Name: journal_stream_idx; Type: INDEX; Schema: compatibility_binary; Owner: postgres
--

CREATE INDEX journal_stream_idx ON compatibility_binary.journal USING btree (stream, version);


--
-- TOC entry 2988 (class 1259 OID 22029)
-- Name: journal_seqnr_idx; Type: INDEX; Schema: compatibility_json; Owner: postgres
--

CREATE INDEX journal_seqnr_idx ON compatibility_json.journal USING btree (seqnr);


--
-- TOC entry 2989 (class 1259 OID 22030)
-- Name: journal_stream_idx; Type: INDEX; Schema: compatibility_json; Owner: postgres
--

CREATE INDEX journal_stream_idx ON compatibility_json.journal USING btree (stream, version);


--
-- TOC entry 2998 (class 1259 OID 22064)
-- Name: journal_seqnr_idx; Type: INDEX; Schema: compatibility_jsonb; Owner: postgres
--

CREATE INDEX journal_seqnr_idx ON compatibility_jsonb.journal USING btree (seqnr);


--
-- TOC entry 2999 (class 1259 OID 22065)
-- Name: journal_stream_idx; Type: INDEX; Schema: compatibility_jsonb; Owner: postgres
--

CREATE INDEX journal_stream_idx ON compatibility_jsonb.journal USING btree (stream, version);


-- Completed on 2022-05-02 17:53:04 +0430

--
-- PostgreSQL database dump complete
--

