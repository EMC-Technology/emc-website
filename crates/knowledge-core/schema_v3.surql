DEFINE TABLE document SCHEMAFULL;
DEFINE FIELD path ON document TYPE string ASSERT string::len($value) > 0 AND string::len($value) <= 4096;
DEFINE FIELD title ON document TYPE option<string>;
DEFINE FIELD source_type ON document TYPE string ASSERT $value IN ['Markdown', 'Code', 'Plain'];
DEFINE FIELD hash ON document TYPE string ASSERT string::len($value) == 64;
DEFINE INDEX doc_hash_idx ON document FIELDS hash UNIQUE;
DEFINE INDEX doc_path_idx ON document FIELDS path;

DEFINE TABLE block SCHEMAFULL;
DEFINE FIELD doc_id ON block TYPE record<document>;
DEFINE FIELD block_type ON block TYPE string ASSERT $value IN ['Heading', 'Paragraph', 'Code', 'List', 'Table'];
DEFINE FIELD start_line ON block TYPE int ASSERT $value >= 0;
DEFINE FIELD end_line ON block TYPE int ASSERT $value >= 0;
DEFINE FIELD embedding ON block TYPE option<array<float>>;
DEFINE FIELD idempotency_key ON block TYPE option<string>;
DEFINE INDEX block_doc_id_idx ON block FIELDS doc_id;
DEFINE INDEX block_pos_idx ON block FIELDS doc_id, start_line, end_line;
DEFINE INDEX block_vec_idx ON block MTREE embedding DIMENSION 2560 DISTANCE COSINE TYPE VECTORFN;
DEFINE INDEX block_idemp_idx ON block FIELDS idempotency_key UNIQUE;

DEFINE TABLE token SCHEMAFULL;
DEFINE FIELD block_id ON token TYPE record<block>;
DEFINE FIELD content ON token TYPE string ASSERT string::len($value) > 0 AND string::len($value) <= 1024;
DEFINE FIELD token_type ON token TYPE string ASSERT $value IN ['Word', 'Punct', 'Symbol', 'Keyword', 'Identifier'];
DEFINE FIELD start_char ON token TYPE int ASSERT $value >= 0;
DEFINE FIELD global_offset ON token TYPE int ASSERT $value >= 0;
DEFINE INDEX token_block_idx ON token FIELDS block_id;
DEFINE INDEX token_global_idx ON token FIELDS global_offset;
DEFINE INDEX token_content_idx ON token FIELDS content;
DEFINE INDEX token_type_idx ON token FIELDS token_type;

DEFINE TABLE references SCHEMAFULL TYPE RELATION FROM ANY TO ANY;
DEFINE FIELD ref_type ON references TYPE string ASSERT $value IN ['Definition', 'Usage', 'Link', 'Inherit', 'Implement', 'Constrain'];
DEFINE FIELD direction ON references TYPE string ASSERT $value IN ['OneWay', 'TwoWay', 'ImplicitTwoWay'];
DEFINE FIELD scope ON references TYPE option<string>;
DEFINE INDEX ref_type_idx ON references FIELDS ref_type, direction;
DEFINE INDEX ref_from_idx ON references FIELDS in;
DEFINE INDEX ref_to_idx ON references FIELDS out;
DEFINE INDEX ref_scope_idx ON references FIELDS scope;

DEFINE TABLE community SCHEMAFULL;
DEFINE FIELD name ON community TYPE string ASSERT string::len($value) > 0 AND string::len($value) <= 512;
DEFINE FIELD cohesion_score ON community TYPE float ASSERT $value >= 0.0 AND $value <= 1.0;
DEFINE FIELD member_ids ON community TYPE array<string> ASSERT array::len($value) > 0;
DEFINE INDEX community_name_idx ON community FIELDS name;
DEFINE INDEX community_cohesion_idx ON community FIELDS cohesion_score;

DEFINE TABLE process SCHEMAFULL;
DEFINE FIELD name ON process TYPE string ASSERT string::len($value) > 0 AND string::len($value) <= 512;
DEFINE FIELD entry_point_id ON process TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD description ON process TYPE option<string>;
DEFINE INDEX process_name_idx ON process FIELDS name;
DEFINE INDEX process_entry_idx ON process FIELDS entry_point_id;

DEFINE TABLE process_step SCHEMAFULL;
DEFINE FIELD process_id ON process_step TYPE record<process>;
DEFINE FIELD symbol_id ON process_step TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD step_order ON process_step TYPE int ASSERT $value >= 0;
DEFINE FIELD confidence ON process_step TYPE float ASSERT $value >= 0.0 AND $value <= 1.0;
DEFINE INDEX step_process_idx ON process_step FIELDS process_id;
DEFINE INDEX step_order_idx ON process_step FIELDS process_id, step_order;

DEFINE TABLE schema_version SCHEMAFULL;
DEFINE FIELD version ON schema_version TYPE string;
DEFINE FIELD applied_at ON schema_version TYPE datetime;
DEFINE FIELD checksum ON schema_version TYPE string;
DEFINE INDEX version_idx ON schema_version FIELDS version UNIQUE;
